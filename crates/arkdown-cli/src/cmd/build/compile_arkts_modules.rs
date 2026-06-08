use super::compile_arkts_handoff::CompileArkTsInfo;
use super::*;
use arkdown_plugin_ohos_resolve::ResolvedImportMap;
use arkdown_resolver::{OhPackage, OhosResolver};

const OHM_PACKAGES_PREFIX: &str = "pkg_modules";

pub(super) struct CompileModuleInfo {
    pub(super) source_path: PathBuf,
    pub(super) source_file: String,
    pub(super) source_file_info: String,
    pub(super) ts_path: PathBuf,
    pub(super) record_name: String,
    pub(super) package_name: String,
    pub(super) module_type: String,
    pub(super) origin_source_lang: String,
    pub(super) is_shared_module: bool,
    pub(super) source_ext: String,
    pub(super) output_ext: String,
}

impl CompileModuleInfo {
    pub(super) fn to_compile_info(&self) -> CompileArkTsInfo {
        CompileArkTsInfo {
            ts_path: self.ts_path.clone(),
            record_name: self.record_name.clone(),
            module_type: self.module_type.clone(),
            source_file: self.source_file_info.clone(),
            package_name: self.package_name.clone(),
            origin_source_lang: self.origin_source_lang.clone(),
            is_shared_module: self.is_shared_module,
            abc_cache_path: None,
        }
    }

    pub(super) fn set_rolldown_commonjs(&mut self, is_commonjs: bool) {
        self.module_type = if is_commonjs { "commonjs" } else { "esm" }.to_string();
    }

    pub(super) fn compute(
        source_path: &Path,
        cache_root: &Path,
        loader_json: &arkdown_profile::LoaderJson,
        config: &ArkdownConfig,
    ) -> Option<CompileModuleInfo> {
        let source_path = canonical_or_owned(&source_path);
        if is_package_manager_module_source(&source_path) {
            return Self::compute_remote(&source_path, cache_root, loader_json);
        }
        let source_ext = source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();
        let output_ext = Self::compile_output_extension(&source_path, &source_ext)?;

        let mut module_roots: Vec<(&str, PathBuf)> = loader_json
            .module_path_map
            .iter()
            .map(|(module_name, module_root)| (module_name.as_str(), PathBuf::from(module_root)))
            .collect();
        module_roots.sort_by(|a, b| b.1.components().count().cmp(&a.1.components().count()));

        for (module_name, module_root) in module_roots {
            let module_root = canonical_or_owned(&module_root);
            let src_main = module_root.join("src/main");

            let source_file = if let Ok(rel) = source_path.strip_prefix(&src_main) {
                format!("src/main/{}", arkdown_path::to_slash(rel))
            } else if let Ok(rel) = source_path.strip_prefix(&module_root) {
                arkdown_path::to_slash(rel)
            } else {
                continue;
            };

            let out_rel = with_extension(&source_file, output_ext);
            let ts_path = cache_root.join(module_name).join(&out_rel);
            let record_rel = Self::record_relative_path(&out_rel);
            let record_name = if module_name == loader_json.module_name {
                format!(
                    "{}/{}/{}",
                    config.product.bundle_name, loader_json.module_name, record_rel
                )
            } else {
                format!(
                    "{}/{}@{}/{}",
                    config.product.bundle_name, loader_json.module_name, module_name, record_rel
                )
            };
            let version = package_version_or_default(&module_root);
            let module_type = Self::module_type_for_source(&source_ext);
            let is_shared_module = module_root_is_shared(&module_root);

            return Some(CompileModuleInfo {
                source_path,
                source_file: source_file.clone(),
                source_file_info: format!(
                    "{}|{}|{}|{}",
                    loader_json.module_name, module_name, version, out_rel
                ),
                ts_path,
                record_name,
                package_name: module_name.to_string(),
                module_type,
                origin_source_lang: Self::origin_source_lang(&source_ext),
                is_shared_module,
                source_ext,
                output_ext: output_ext.to_string(),
            });
        }

        Self::compute_remote(&source_path, cache_root, loader_json)
    }

    fn compute_remote(
        source_path: &Path,
        cache_root: &Path,
        loader_json: &arkdown_profile::LoaderJson,
    ) -> Option<CompileModuleInfo> {
        let source_ext = source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();
        let output_ext = Self::compile_output_extension(&source_path, &source_ext)?;
        let package_root = find_package_root(source_path)?;
        let package_name = package_name_or_dir(&package_root);
        let package_version = package_version_or_default(&package_root);
        let package_entry_name = pkg_modules_package_name(&package_root, loader_json)?;
        let rel_to_pkg = arkdown_path::to_slash(source_path.strip_prefix(&package_root).ok()?);
        let out_rel = with_extension(&rel_to_pkg, output_ext);

        let project_root = canonical_or_owned(Path::new(&loader_json.project_root_path));
        let cache_rel = source_path.strip_prefix(&project_root).ok()?;
        let ts_path = cache_root.join(with_extension(
            &arkdown_path::to_slash(cache_rel),
            output_ext,
        ));
        let record_name = format!(
            "{}/{}",
            package_entry_name,
            out_rel
                .trim_end_matches(".ets")
                .trim_end_matches(".ts")
                .trim_end_matches(".js")
        );

        Some(CompileModuleInfo {
            source_path: source_path.to_path_buf(),
            source_file: rel_to_pkg,
            source_file_info: format!(
                "{}|{}|{}|{}",
                loader_json.module_name, package_name, package_version, out_rel
            ),
            ts_path,
            record_name,
            package_name: package_entry_name,
            module_type: Self::module_type_for_source(&source_ext),
            origin_source_lang: Self::origin_source_lang(&source_ext),
            is_shared_module: module_root_is_shared(&package_root),
            source_ext,
            output_ext: output_ext.to_string(),
        })
    }
}

impl CompileModuleInfo {
    fn compile_output_extension(source_path: &Path, source_ext: &str) -> Option<&'static str> {
        match source_ext {
            "ets" => Some(if Self::should_emit_js_for_ts_or_ets(source_path) {
                "js"
            } else {
                "ts"
            }),
            "ts" => Some(if Self::should_emit_js_for_ts_or_ets(source_path) {
                "js"
            } else {
                "ts"
            }),
            "js" | "mjs" | "cjs" => Some("js"),
            "json" | "json5" => Some("json"),
            _ => None,
        }
    }

    fn should_emit_js_for_ts_or_ets(source_path: &Path) -> bool {
        let Ok(source) = fs_err::read_to_string(source_path) else {
            return false;
        };
        if Self::has_keep_ts_comment(&source) {
            return false;
        }
        Self::has_ts_no_check_or_ignore_comment(&source)
    }

    fn has_keep_ts_comment(source: &str) -> bool {
        source
            .lines()
            .next()
            .is_some_and(|line| line.trim() == "// @keepTs")
    }

    fn has_ts_no_check_or_ignore_comment(source: &str) -> bool {
        source
            .lines()
            .any(|line| line.contains("@ts-nocheck") || line.contains("@ts-ignore"))
    }

    fn module_type_for_source(source_ext: &str) -> String {
        let is_commonjs = source_ext == "cjs";
        if is_commonjs { "commonjs" } else { "esm" }.to_string()
    }

    fn origin_source_lang(source_ext: &str) -> String {
        match source_ext {
            "ets" => "ets",
            "ts" => "ts",
            "js" | "mjs" | "cjs" => "js",
            _ => "",
        }
        .to_string()
    }

    fn record_relative_path(ts_rel: &str) -> String {
        let rel = ts_rel
            .trim_start_matches("src/main/")
            .trim_end_matches(".ets")
            .trim_end_matches(".ts")
            .trim_end_matches(".js");
        rel.to_string()
    }
}

fn source_path_if_compilable(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() || !path.exists() {
        return None;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if file_name.ends_with(".d.ts") || file_name.ends_with(".d.ets") {
        return None;
    }
    let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    if !matches!(ext, "ets" | "ts" | "js" | "mjs" | "cjs" | "json" | "json5") {
        return None;
    }
    Some(canonical_or_owned(path))
}

pub(super) fn module_id_to_source_path(module_id: &str) -> Option<PathBuf> {
    if module_id.starts_with('\0') || module_id.starts_with("virtual:") {
        return None;
    }
    let module_id = module_id
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(module_id);
    source_path_if_compilable(Path::new(module_id))
}

pub(super) struct CompileModuleImportResolver<'a> {
    cache_root: &'a Path,
    loader_json: &'a arkdown_profile::LoaderJson,
    config: &'a ArkdownConfig,
    resolved_imports: &'a ResolvedImportMap,
    ohos_resolver: Option<&'a OhosResolver>,
}

impl<'a> CompileModuleImportResolver<'a> {
    pub(super) fn new(
        cache_root: &'a Path,
        loader_json: &'a arkdown_profile::LoaderJson,
        config: &'a ArkdownConfig,
        resolved_imports: &'a ResolvedImportMap,
        ohos_resolver: Option<&'a OhosResolver>,
    ) -> Self {
        Self {
            cache_root,
            loader_json,
            config,
            resolved_imports,
            ohos_resolver,
        }
    }

    pub(super) fn rewrite_source(&self, source: &str, source_id: &Path, importer: &Path) -> String {
        rewrite_value_static_module_specifiers_with(
            source,
            &source_id.to_string_lossy(),
            |specifier| self.ohm_for_specifier(specifier, importer),
        )
    }

    fn ohm_for_specifier(&self, specifier: &str, importer: &Path) -> Option<String> {
        if let Some(ohm_url) = system_api_or_lib_ohm_url(specifier, self.loader_json, self.config) {
            return Some(ohm_url);
        }

        if is_ohm_url_specifier(specifier) {
            return None;
        }

        let resolved = self.resolve_specifier(specifier, importer)?;
        let info =
            CompileModuleInfo::compute(&resolved, self.cache_root, self.loader_json, self.config)?;
        Some(compile_module_ohm_url(&info))
    }

    fn resolve_specifier(&self, specifier: &str, importer: &Path) -> Option<PathBuf> {
        if let Some(resolved) =
            resolve_compile_module_specifier_from_map(specifier, importer, self.resolved_imports)
        {
            return Some(resolved);
        }

        let resolved = self.ohos_resolver?.resolve(specifier, importer).ok()?;
        let path = resolved.local_path()?;
        source_path_if_compilable(path)
    }
}

pub(super) fn rewrite_compile_module_imports_to_ohm(
    source: &str,
    source_id: &Path,
    importer: &Path,
    import_resolver: &CompileModuleImportResolver<'_>,
) -> String {
    import_resolver.rewrite_source(source, source_id, importer)
}

#[cfg(test)]
fn compile_module_ohm_for_specifier(
    specifier: &str,
    importer: &Path,
    cache_root: &Path,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
    resolved_imports: &ResolvedImportMap,
) -> Option<String> {
    CompileModuleImportResolver::new(cache_root, loader_json, config, resolved_imports, None)
        .ohm_for_specifier(specifier, importer)
}

fn system_api_or_lib_ohm_url(
    specifier: &str,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
) -> Option<String> {
    let request = specifier.trim();
    if let Some(lib_key) = request
        .strip_prefix("lib")
        .and_then(|rest| rest.strip_suffix(".so"))
        .filter(|key| !key.is_empty())
    {
        return Some(format!(
            "@app:{}/{}/{}",
            config.product.bundle_name, loader_json.module_name, lib_key
        ));
    }

    let (module_type, system_key) = request.strip_prefix('@')?.split_once('.')?;
    if !matches!(module_type, "system" | "ohos" | "arkts" | "arkui-x") || system_key.is_empty() {
        return None;
    }

    let system_module = format!("{module_type}.{system_key}");
    if is_native_system_module(&system_module) {
        return Some(format!("@native:{system_module}"));
    }
    if module_type == "arkts" {
        return Some(format!("@ohos:{system_module}"));
    }
    Some(format!("@ohos:{system_key}"))
}

fn is_native_system_module(system_module: &str) -> bool {
    matches!(
        system_module,
        "system.app"
            | "ohos.app"
            | "system.router"
            | "system.curves"
            | "ohos.curves"
            | "system.matrix4"
            | "ohos.matrix4"
    )
}

fn is_ohm_url_specifier(specifier: &str) -> bool {
    // Upstream `resolve_ohm_url.ts::isOhmUrl()` treats every `@xxx:` request
    // as an already-normalized OHM URL.
    specifier.starts_with('@')
        && specifier
            .split('/')
            .next()
            .is_some_and(|head| head.contains(':'))
}

fn compile_module_ohm_url(info: &CompileModuleInfo) -> String {
    // Upstream `ModuleSourceFile.getOhmUrl()` prefixes record names beginning
    // with `PACKAGES` (`pkg_modules`) as package OHM URLs; all other source
    // records are bundle OHM URLs.
    if info.record_name.starts_with(OHM_PACKAGES_PREFIX) {
        format!("@package:{}", info.record_name)
    } else {
        format!("@bundle:{}", info.record_name)
    }
}

fn resolve_compile_module_specifier_from_map(
    specifier: &str,
    importer: &Path,
    resolved_imports: &ResolvedImportMap,
) -> Option<PathBuf> {
    let resolved = resolved_import_from_map(specifier, importer, resolved_imports)?;
    module_id_to_source_path(&resolved)
}

fn resolved_import_from_map(
    specifier: &str,
    importer: &Path,
    resolved_imports: &ResolvedImportMap,
) -> Option<String> {
    let importer_keys = [
        importer.to_string_lossy().into_owned(),
        slash_path(importer),
        canonical_or_owned(importer).to_string_lossy().into_owned(),
        slash_path(&canonical_or_owned(importer)),
    ];
    importer_keys
        .iter()
        .find_map(|key| resolved_imports.get(key))
        .and_then(|imports| imports.get(specifier))
        .cloned()
}

pub(super) fn compile_source_passthrough(source: &str, source_ext: &str) -> String {
    match source_ext {
        "json" | "json5" => {
            let value: serde_json::Value = if source_ext == "json5" {
                json5::from_str(source).unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::from_str(source).unwrap_or(serde_json::Value::Null)
            };
            format!(
                "export default {};\n",
                serde_json::to_string(&value).unwrap_or_default()
            )
        }
        _ => source.to_string(),
    }
}

pub(super) fn find_package_root(source_path: &Path) -> Option<PathBuf> {
    if let Some(package_root) = package_root_from_modules_dir(source_path) {
        return Some(package_root);
    }

    let mut dir = source_path.parent()?;
    loop {
        if dir.join("oh-package.json5").is_file() || dir.join("package.json").is_file() {
            return Some(dir.to_path_buf());
        }
        let parent = dir.parent()?;
        if parent == dir {
            return None;
        }
        dir = parent;
    }
}

fn package_root_from_modules_dir(source_path: &Path) -> Option<PathBuf> {
    let components: Vec<_> = source_path.components().collect();
    let mut modules_index = None;
    for (index, component) in components.iter().enumerate() {
        let name = component.as_os_str().to_str()?;
        if name == "oh_modules" || name == "node_modules" {
            modules_index = Some(index);
        }
    }

    let modules_index = modules_index?;
    let first = components.get(modules_index + 1)?.as_os_str().to_str()?;
    let package_end = if first.starts_with('@') {
        modules_index + 3
    } else {
        modules_index + 2
    };
    if package_end > components.len() {
        return None;
    }

    let mut root = PathBuf::new();
    for component in &components[..package_end] {
        root.push(component.as_os_str());
    }
    Some(root)
}

fn is_package_manager_module_source(source_path: &Path) -> bool {
    package_root_from_modules_dir(source_path).is_some()
}

pub(super) fn package_name_or_dir(package_root: &Path) -> String {
    OhPackage::load_from_root(package_root)
        .and_then(|pkg| pkg.name().map(str::to_string))
        .unwrap_or_else(|| {
            package_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string()
        })
}

pub(super) fn package_version_or_default(package_root: &Path) -> String {
    let v = OhPackage::load_from_root(package_root)
        .map(|p| p.version().to_string())
        .unwrap_or_default();
    if v.is_empty() {
        "1.0.0".to_string()
    } else {
        v
    }
}

fn module_root_is_shared(module_root: &Path) -> bool {
    arkdown_config::module_json::RawModuleJson::load(module_root)
        .map(|module_json| {
            matches!(
                OhosModuleType::from_type_str(&module_json.module.module_type),
                OhosModuleType::Shared
            )
        })
        .unwrap_or(false)
}

pub(super) fn pkg_modules_package_name(
    package_root: &Path,
    loader_json: &arkdown_profile::LoaderJson,
) -> Option<String> {
    let project_root = canonical_or_owned(Path::new(&loader_json.project_root_path));
    let package_dir = if loader_json.package_manager_type == "ohpm" {
        "oh_modules"
    } else {
        "node_modules"
    };

    if let Ok(rel) = package_root.strip_prefix(project_root.join(package_dir)) {
        return Some(format!(
            "pkg_modules/{}",
            arkdown_path::to_slash(rel).replace(package_dir, "pkg_modules")
        ));
    }

    for (module_name, module_root) in &loader_json.module_path_map {
        let module_root = canonical_or_owned(Path::new(module_root));
        if let Ok(rel) = package_root.strip_prefix(module_root.join(package_dir)) {
            return Some(format!(
                "pkg_modules@{}/{}",
                module_name,
                arkdown_path::to_slash(rel).replace(package_dir, "pkg_modules")
            ));
        }
    }

    None
}

/// Convert a path to a forward-slash string.
///
/// Per SPEC-048: delegates to `arkdown_path::to_slash()`.
pub(super) fn slash_path(path: &Path) -> String {
    arkdown_path::to_slash(path)
}

pub(super) fn package_entry_path(package_root: &Path) -> Option<PathBuf> {
    let pkg = OhPackage::load_from_root(package_root);
    let request = pkg
        .as_ref()
        .and_then(|p| p.exports_import_field())
        .or_else(|| {
            pkg.as_ref()
                .and_then(|p| p.main_field().map(str::to_string))
        })
        .unwrap_or_else(|| "index".to_string());
    resolve_package_request(package_root, &request)
        .or_else(|| resolve_package_request(package_root, "index"))
}

fn resolve_package_request(package_root: &Path, request: &str) -> Option<PathBuf> {
    let request = arkdown_path::strip_relative_prefix(request);
    let base = package_root.join(request);
    if base.is_file() {
        return Some(base);
    }
    for ext in ["ets", "ts", "js", "mjs", "cjs", "json"] {
        let candidate = base.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    for index in [
        "index.ets",
        "index.ts",
        "index.js",
        "index.mjs",
        "index.cjs",
    ] {
        let candidate = base.join(index);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub(super) fn symlink_pkg_module_aliases(
    package_root: &Path,
    loader_json: &arkdown_profile::LoaderJson,
) -> Vec<String> {
    let package_root = canonical_or_owned(package_root);
    let mut aliases = Vec::new();
    let mut roots = vec![Path::new(&loader_json.project_root_path).join("oh_modules")];
    for module_root in loader_json.module_path_map.values() {
        roots.push(Path::new(module_root).join("oh_modules"));
    }
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(8)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            let Ok(file_type) = std::fs::symlink_metadata(entry.path()).map(|m| m.file_type())
            else {
                continue;
            };
            if !file_type.is_symlink() {
                continue;
            }
            let entry_path = arkdown_path::to_slash(entry.path());
            if entry_path.contains("/oh_modules/.ohpm/oh_modules/") {
                continue;
            }
            if canonical_or_owned(entry.path()) != package_root {
                continue;
            }
            if let Some(alias) = pkg_modules_package_name(entry.path(), loader_json) {
                aliases.push(alias);
            }
        }
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

pub(super) fn strip_source_extension(path: &str) -> &str {
    for ext in [".ets", ".ts", ".js", ".mjs", ".cjs", ".json", ".json5"] {
        if let Some(stripped) = path.strip_suffix(ext) {
            return stripped;
        }
    }
    path
}

pub(super) fn with_extension(path: &str, extension: &str) -> String {
    path.rsplit_once('.')
        .map(|(stem, _)| format!("{stem}.{extension}"))
        .unwrap_or_else(|| format!("{path}.{extension}"))
}

#[cfg(test)]
mod tests {
    use arkdown_core::{
        ArkChunk, ArkdownConfig, BuildMode, BuildOption, ChunkGraph, ModuleConfig, OhosModuleType,
        OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
    };
    use fs_err as fs;
    use oxc_index::IndexVec;
    use rustc_hash::FxHashMap;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn all_module_ids_prefers_full_rolldown_module_graph() {
        let graph = ChunkGraph {
            chunks: vec![ArkChunk {
                module_ids: vec!["/project/entry/src/main/ets/Index.ets".to_string()],
                ..Default::default()
            }]
            .into_iter()
            .collect::<IndexVec<_, _>>(),
            full_module_ids: vec![
                "/project/entry/src/main/ets/Index.ets".to_string(),
                "/project/entry/src/main/ets/PureReexport.ts".to_string(),
            ],
            full_module_is_commonjs: FxHashMap::default(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        };

        assert_eq!(
            graph.all_module_ids(),
            vec![
                "/project/entry/src/main/ets/Index.ets".to_string(),
                "/project/entry/src/main/ets/PureReexport.ts".to_string(),
            ]
        );
    }

    #[test]
    fn module_type_uses_rolldown_commonjs_result_not_source_scan() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();
        let phone_root = project.join("phone");
        let source_path = phone_root.join("src/main/ets/cjsish.js");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(&source_path, "module.exports = function cjsish() {};\n").unwrap();

        let loader_json = make_loader_json(project, &[("phone", &phone_root)]);
        let config = make_config(project, &phone_root);
        let cache_root = project.join("cache");

        let mut info = CompileModuleInfo::compute(&source_path, &cache_root, &loader_json, &config)
            .expect("module info");

        assert_eq!(
            info.module_type, "esm",
            "CompileArkTS must not rescan source text to infer CommonJS"
        );

        info.set_rolldown_commonjs(true);
        assert_eq!(info.module_type, "commonjs");
    }

    #[test]
    fn compile_module_info_reads_shared_module_flag_from_module_json() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();
        let shared_root = project.join("shared");
        let source_path = shared_root.join("src/main/ets/pages/Index.ets");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(&source_path, "export const value = 1;\n").unwrap();
        fs::write(
            shared_root.join("src/main/module.json5"),
            r#"{ module: { name: "shared", type: "shared" } }"#,
        )
        .unwrap();

        let loader_json = make_loader_json(project, &[("shared", &shared_root)]);
        let config = make_config(project, &shared_root);
        let cache_root = project.join("cache");

        let info = CompileModuleInfo::compute(&source_path, &cache_root, &loader_json, &config)
            .expect("module info");

        assert!(
            info.is_shared_module,
            "filesInfo isSharedModule follows module.json5 module.type"
        );
        assert!(info.to_compile_info().is_shared_module);
    }

    #[test]
    fn compile_module_ohm_uses_package_prefix_for_ohpm_package_records() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();
        let phone_root = project.join("phone");
        let importer = phone_root.join("src/main/ets/agc/Request.ts");
        fs::create_dir_all(importer.parent().unwrap()).unwrap();
        fs::write(&importer, "").unwrap();

        let package_root = project
            .join("oh_modules/.ohpm/@hw-agconnect+cloud@1.0.2/oh_modules/@hw-agconnect/cloud");
        fs::create_dir_all(&package_root).unwrap();
        fs::write(
            package_root.join("oh-package.json5"),
            r#"{"name":"@hw-agconnect/cloud","version":"1.0.2","main":"index.ets"}"#,
        )
        .unwrap();
        fs::write(package_root.join("index.ets"), "export default {};\n").unwrap();

        let loader_json = make_loader_json(project, &[("phone", &phone_root)]);
        let config = make_config(project, &phone_root);
        let cache_root = project.join("cache");
        let resolved_imports = make_resolved_imports(
            &importer,
            "@hw-agconnect/cloud",
            &package_root.join("index.ets"),
        );

        let ohm = compile_module_ohm_for_specifier(
            "@hw-agconnect/cloud",
            &importer,
            &cache_root,
            &loader_json,
            &config,
            &resolved_imports,
        )
        .unwrap();

        assert_eq!(
            ohm,
            "@package:pkg_modules/.ohpm/@hw-agconnect+cloud@1.0.2/pkg_modules/@hw-agconnect/cloud/index"
        );
    }

    #[test]
    fn package_manager_sources_keep_pkg_modules_record_even_in_module_path_map() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();
        let phone_root = project.join("phone");
        let package_root = project
            .join("oh_modules/.ohpm/@hw-agconnect+cloud@1.0.2/oh_modules/@hw-agconnect/cloud");
        let source_path = package_root.join("src/main/ets/auth/Auth.ets");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(
            package_root.join("oh-package.json5"),
            r#"{"name":"@hw-agconnect/cloud","version":"1.0.2","main":"index.ets"}"#,
        )
        .unwrap();
        fs::write(&source_path, "export class Auth {}\n").unwrap();

        let loader_json = make_loader_json(
            project,
            &[
                ("phone", &phone_root),
                ("@hw-agconnect/cloud", &package_root),
            ],
        );
        let config = make_config(project, &phone_root);
        let cache_root = project.join("cache");

        let info = CompileModuleInfo::compute(&source_path, &cache_root, &loader_json, &config)
            .expect("package module info");

        assert_eq!(
            info.record_name,
            "pkg_modules/.ohpm/@hw-agconnect+cloud@1.0.2/pkg_modules/@hw-agconnect/cloud/src/main/ets/auth/Auth"
        );
        assert_eq!(
            compile_module_ohm_url(&info),
            "@package:pkg_modules/.ohpm/@hw-agconnect+cloud@1.0.2/pkg_modules/@hw-agconnect/cloud/src/main/ets/auth/Auth"
        );
        assert_eq!(
            info.source_file_info,
            "phone|@hw-agconnect/cloud|1.0.2|src/main/ets/auth/Auth.ts"
        );
    }

    #[test]
    fn compile_module_ohm_keeps_bundle_prefix_for_project_module_records() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();
        let phone_root = project.join("phone");
        let utils_root = project.join("utils");
        let importer = phone_root.join("src/main/ets/pages/Index.ets");
        let logger = utils_root.join("src/main/ets/Index.ets");
        fs::create_dir_all(importer.parent().unwrap()).unwrap();
        fs::create_dir_all(logger.parent().unwrap()).unwrap();
        fs::write(&importer, "").unwrap();
        fs::write(&logger, "export class Logger {}\n").unwrap();
        fs::write(
            project.join("oh-package.json5"),
            r#"{"name":"hmosworld","dependencies":{"@ohos/utils":"file:./utils"}}"#,
        )
        .unwrap();
        fs::write(
            utils_root.join("oh-package.json5"),
            r#"{"name":"@ohos/utils","version":"1.0.0","main":"src/main/ets/Index.ets"}"#,
        )
        .unwrap();

        let loader_json =
            make_loader_json(project, &[("phone", &phone_root), ("utils", &utils_root)]);
        let config = make_config(project, &phone_root);
        let cache_root = project.join("cache");
        let resolved_imports = make_resolved_imports(&importer, "@ohos/utils", &logger);

        let ohm = compile_module_ohm_for_specifier(
            "@ohos/utils",
            &importer,
            &cache_root,
            &loader_json,
            &config,
            &resolved_imports,
        )
        .unwrap();

        assert_eq!(ohm, "@bundle:com.example.hmosworld/phone@utils/ets/Index");
    }

    #[test]
    fn compile_module_ohm_resolves_scoped_local_package_when_map_misses() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();
        let phone_root = project.join("products/phone");
        let mine_root = project.join("features/mine");
        let datastore_root = project.join("commons/datastore");
        let importer = mine_root.join("src/main/ets/model/UserModel.ets");
        let datastore_entry = datastore_root.join("Index.ets");

        fs::create_dir_all(importer.parent().unwrap()).unwrap();
        fs::create_dir_all(&datastore_root).unwrap();
        fs::write(&importer, "").unwrap();
        fs::write(&datastore_entry, "export class PreferenceManager {}\n").unwrap();
        fs::write(
            mine_root.join("oh-package.json5"),
            r#"{"name":"mine","dependencies":{"@ohos/datastore":"file:../../commons/datastore"}}"#,
        )
        .unwrap();
        fs::write(
            datastore_root.join("oh-package.json5"),
            r#"{"name":"datastore","version":"1.0.0","main":"Index.ets"}"#,
        )
        .unwrap();

        let loader_json = make_loader_json(
            project,
            &[
                ("phone", &phone_root),
                ("mine", &mine_root),
                ("datastore", &datastore_root),
            ],
        );
        let config = make_config(project, &phone_root);
        let cache_root = project.join("cache");
        let resolved_imports = ResolvedImportMap::default();
        let ohos_resolver = OhosResolver::new(arkdown_resolver::ResolverConfig {
            project_root: project.to_path_buf(),
            sdk_api_dirs: vec![],
            sdk_kits_dirs: vec![],
            pkg_name_to_pkg_info: FxHashMap::default(),
            use_normalized_ohm_url: false,
            case_sensitive_check: false,
            oh_modules_roots: vec![mine_root.join("oh_modules"), project.join("oh_modules")],
            source_set_overlays: vec![],
        });
        let import_resolver = CompileModuleImportResolver::new(
            &cache_root,
            &loader_json,
            &config,
            &resolved_imports,
            Some(&ohos_resolver),
        );

        let ohm = import_resolver
            .ohm_for_specifier("@ohos/datastore", &importer)
            .unwrap();
        assert_eq!(ohm, "@bundle:com.example.hmosworld/phone@datastore/Index");

        let rewritten = import_resolver.rewrite_source(
            "import { PreferenceManager } from '@ohos/datastore';\nimport type { UserAccount } from '@ohos/login';\n",
            &cache_root.join("mine/src/main/ets/model/UserModel.ts"),
            &importer,
        );
        assert!(rewritten.contains("from \"@bundle:com.example.hmosworld/phone@datastore/Index\""));
        assert!(rewritten.contains("from '@ohos/login'"));
    }

    #[test]
    fn compile_module_ohm_rewrites_system_api_and_lib_requests() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();
        let phone_root = project.join("phone");
        let importer = phone_root.join("src/main/ets/pages/Index.ets");
        fs::create_dir_all(importer.parent().unwrap()).unwrap();
        fs::write(&importer, "").unwrap();

        let loader_json = make_loader_json(project, &[("phone", &phone_root)]);
        let config = make_config(project, &phone_root);
        let cache_root = project.join("cache");
        let resolved_imports = ResolvedImportMap::default();

        let cases = [
            ("@ohos.app.ability.common", "@ohos:app.ability.common"),
            ("@ohos.hilog", "@ohos:hilog"),
            ("@system.app", "@native:system.app"),
            ("@arkts.collections", "@ohos:arkts.collections"),
            (
                "libapplication.so",
                "@app:com.example.hmosworld/phone/application",
            ),
        ];
        for (specifier, expected) in cases {
            let ohm = compile_module_ohm_for_specifier(
                specifier,
                &importer,
                &cache_root,
                &loader_json,
                &config,
                &resolved_imports,
            )
            .unwrap();
            assert_eq!(ohm, expected, "specifier {specifier}");
        }
    }

    #[test]
    fn compile_module_ohm_does_not_guess_without_rolldown_resolved_import() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();
        let phone_root = project.join("phone");
        let importer = phone_root.join("src/main/ets/pages/Index.ets");
        let target = phone_root.join("src/main/ets/pages/Dep.ets");
        fs::create_dir_all(importer.parent().unwrap()).unwrap();
        fs::write(&importer, "").unwrap();
        fs::write(&target, "export const dep = 1;\n").unwrap();

        let loader_json = make_loader_json(project, &[("phone", &phone_root)]);
        let config = make_config(project, &phone_root);
        let cache_root = project.join("cache");
        let resolved_imports = ResolvedImportMap::default();

        let ohm = compile_module_ohm_for_specifier(
            "./Dep",
            &importer,
            &cache_root,
            &loader_json,
            &config,
            &resolved_imports,
        );

        assert!(ohm.is_none());
    }

    fn make_resolved_imports(
        importer: &Path,
        specifier: &str,
        resolved: &Path,
    ) -> ResolvedImportMap {
        let mut imports = FxHashMap::default();
        imports.insert(
            specifier.to_string(),
            resolved.to_string_lossy().into_owned(),
        );
        let mut map = FxHashMap::default();
        map.insert(importer.to_string_lossy().into_owned(), imports);
        map
    }

    fn make_loader_json(project: &Path, modules: &[(&str, &Path)]) -> arkdown_profile::LoaderJson {
        let mut loader_json = arkdown_profile::LoaderJson {
            module_path_map: Default::default(),
            compile_mode: "esmodule".to_string(),
            project_root_path: project.to_string_lossy().into_owned(),
            node_modules_path: project
                .join("loader_out/node_modules")
                .to_string_lossy()
                .into_owned(),
            byte_code_har_info: FxHashMap::default(),
            declaration_entry: vec![],
            module_name: "phone".to_string(),
            hsp_name_ohm_map: FxHashMap::default(),
            har_name_ohm_map: FxHashMap::default(),
            package_manager_type: "ohpm".to_string(),
            hvigor_compile_entry: vec![],
            other_compile_files: vec![],
            dynamic_import_lib_info: Default::default(),
            router_map: vec![],
            hsp_resources_map: FxHashMap::default(),
            update_version_info: FxHashMap::default(),
            an_build_out_put: Some(project.join("an").to_string_lossy().into_owned()),
            an_build_mode: Some("type".to_string()),
            ap_path: None,
            build_config_path: None,
            patch_config: None,
            non_entry_importees: vec![],
            compiled_importees: vec![],
            compile_entry: vec![],
            module_root_path: modules[0].1.join("src/main").to_string_lossy().into_owned(),
            cache_path: project.join("cache").to_string_lossy().into_owned(),
            ace_module_build: project.join("ets").to_string_lossy().into_owned(),
            build_mode: "release".to_string(),
            support_chunks: false,
            use_normalized_ohm_url: false,
            api_version: 12,
            fallback_an_build: false,
            pkg_name_to_pkg_brief_info: FxHashMap::default(),
            workers: vec![],
        };
        for (name, root) in modules {
            loader_json
                .module_path_map
                .insert((*name).to_string(), root.to_string_lossy().into_owned());
        }
        loader_json
    }

    fn make_config(project: &Path, phone_root: &Path) -> ArkdownConfig {
        ArkdownConfig {
            project_root: project.to_path_buf(),
            build_mode: BuildMode::Release,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.hmosworld".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                compatible_sdk_version_stage: None,
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: false,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                    preload_system_so: false,
                    preload_so: vec![],
                    remove_permissions: vec![],
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: vec![],
                },
                version_code: None,
                version_name: None,
            },
            modules: vec![ModuleConfig {
                name: "phone".to_string(),
                src_path: phone_root.to_path_buf(),
                module_type: OhosModuleType::Entry,
            }],
            sdk_config: SdkConfig {
                ets_loader_path: project.join("sdk/ets/build-tools/ets-loader"),
                es2abc_path: project.join("sdk/ets/build-tools/es2abc"),
                restool_path: project.join("sdk/toolchains/restool"),
                compile_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        }
    }
}
