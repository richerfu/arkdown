use super::compile_arkts_handoff::CompileArkTsInfo;
use super::*;
use arkdown_resolver::OhPackage;

pub(super) struct CompileModuleInfo {
    pub(super) source_path: PathBuf,
    pub(super) source_file: String,
    pub(super) source_file_info: String,
    pub(super) ts_path: PathBuf,
    pub(super) record_name: String,
    pub(super) package_name: String,
    pub(super) module_type: String,
    pub(super) origin_source_lang: String,
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
            abc_cache_path: None,
        }
    }


    pub(super) fn compute(
        source_path: &Path,
        cache_root: &Path,
        loader_json: &arkdown_profile::LoaderJson,
        config: &ArkdownConfig,
    ) -> Option<CompileModuleInfo> {
        let source_path = canonical_or_owned(&source_path);
        let source_ext = source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();
        let output_ext = compile_output_extension(&source_path, &source_ext)?;
    
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
                format!("src/main/{}", slash_path(rel))
            } else if let Ok(rel) = source_path.strip_prefix(&module_root) {
                slash_path(rel)
            } else {
                continue;
            };
    
            let out_rel = with_extension(&source_file, output_ext);
            let ts_path = cache_root.join(module_name).join(&out_rel);
            let record_rel = record_relative_path(&out_rel);
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
            let module_type = module_type_for_source(&source_path, &source_ext);
    
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
                origin_source_lang: origin_source_lang(&source_ext),
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
        let output_ext = compile_output_extension(&source_path, &source_ext)?;
        let package_root = find_package_root(source_path)?;
        let package_name = package_name_or_dir(&package_root);
        let package_version = package_version_or_default(&package_root);
        let package_entry_name = pkg_modules_package_name(&package_root, loader_json)?;
        let rel_to_pkg = slash_path(source_path.strip_prefix(&package_root).ok()?);
        let out_rel = with_extension(&rel_to_pkg, output_ext);
    
        let project_root = canonical_or_owned(Path::new(&loader_json.project_root_path));
        let cache_rel = source_path.strip_prefix(&project_root).ok()?;
        let ts_path = cache_root.join(with_extension(&slash_path(cache_rel), output_ext));
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
            module_type: module_type_for_source(source_path, &source_ext),
            origin_source_lang: origin_source_lang(&source_ext),
            source_ext,
            output_ext: output_ext.to_string(),
        })
    }
}

pub(super) fn compile_module_ids(chunk_graph: &ChunkGraph) -> Vec<String> {
    let mut seen = FxHashSet::default();
    let mut ids = Vec::new();

    if !chunk_graph.full_module_ids.is_empty() {
        for module_id in &chunk_graph.full_module_ids {
            if seen.insert(module_id.clone()) {
                ids.push(module_id.clone());
            }
        }
        return ids;
    }

    for chunk in &chunk_graph.chunks {
        for module_id in &chunk.module_ids {
            if seen.insert(module_id.clone()) {
                ids.push(module_id.clone());
            }
        }
    }
    ids
}

pub(super) fn module_id_to_source_path(module_id: &str) -> Option<PathBuf> {
    if module_id.starts_with('\0') || module_id.starts_with("virtual:") {
        return None;
    }
    let module_id = module_id
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(module_id);
    let path = Path::new(module_id);
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



fn compile_output_extension(source_path: &Path, source_ext: &str) -> Option<&'static str> {
    match source_ext {
        "ets" => Some(if should_emit_js_for_ts_or_ets(source_path) {
            "js"
        } else {
            "ts"
        }),
        "ts" => Some(if should_emit_js_for_ts_or_ets(source_path) {
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
    if has_keep_ts_comment(&source) {
        return false;
    }
    has_ts_no_check_or_ignore_comment(&source)
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

fn module_type_for_source(source_path: &Path, source_ext: &str) -> String {
    let is_commonjs = match source_ext {
        "cjs" => true,
        "mjs" | "ets" | "ts" => false,
        "js" => fs_err::read_to_string(source_path)
            .map(|source| {
                let package_has_module_field = find_package_root(source_path)
                    .and_then(|root| {
                        OhPackage::load_from_root(&root)
                            .and_then(|p| p.module.as_deref().map(str::to_string))
                    })
                    .is_some();
                !package_has_module_field
                    && (source.contains("module.exports")
                        || source.contains("exports.")
                        || source.contains("= exports")
                        || source.contains("require("))
            })
            .unwrap_or(false),
        _ => false,
    };
    if is_commonjs { "commonjs" } else { "esm" }.to_string()
}

pub(super) fn rewrite_compile_module_imports_to_ohm(
    source: &str,
    importer: &Path,
    cache_root: &Path,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
) -> String {
    rewrite_static_module_specifiers_with(source, &importer.to_string_lossy(), |specifier| {
        compile_module_ohm_for_specifier(specifier, importer, cache_root, loader_json, config)
    })
}

fn compile_module_ohm_for_specifier(
    specifier: &str,
    importer: &Path,
    cache_root: &Path,
    loader_json: &arkdown_profile::LoaderJson,
    config: &ArkdownConfig,
) -> Option<String> {
    if specifier.starts_with("@bundle:")
        || specifier.starts_with("@shared:")
        || specifier.starts_with("@ohos:")
        || specifier.starts_with("@system.")
        || specifier.starts_with("@native:")
    {
        return None;
    }

    let resolved = if specifier.starts_with('.') {
        resolve_source_request(importer.parent()?, specifier)?
    } else {
        local_entry_for_package_specifier(specifier, importer, loader_json)?
    };
    let info = CompileModuleInfo::compute(&resolved, cache_root, loader_json, config)?;
    Some(format!("@bundle:{}", info.record_name))
}

fn local_entry_for_package_specifier(
    specifier: &str,
    importer: &Path,
    loader_json: &arkdown_profile::LoaderJson,
) -> Option<PathBuf> {
    if let Some(dynamic_info) = loader_json.dynamic_import_lib_info.get(specifier) {
        let entry_file = dynamic_info
            .get("entryFilePath")
            .and_then(|value| value.as_str())?;
        return Some(canonical_or_owned(Path::new(entry_file)));
    }

    if let Some(importer_root) = find_package_root(importer) {
        if let Some(dep_value) = read_manifest_dependency_value(&importer_root, specifier) {
            if let Some(local_dep) = dep_value.strip_prefix("file:") {
                let dep_root = canonical_or_owned(&importer_root.join(local_dep));
                if let Some(entry) = package_entry_path(&dep_root) {
                    return Some(canonical_or_owned(&entry));
                }
            }
        }
    }

    for module_root in loader_json.module_path_map.values() {
        let module_root = canonical_or_owned(Path::new(module_root));
        if package_name_or_dir(&module_root) != specifier {
            continue;
        }
        if let Some(entry) = package_entry_path(&module_root) {
            return Some(canonical_or_owned(&entry));
        }
    }
    None
}

pub(super) fn read_manifest_dependency_value(
    package_root: &Path,
    specifier: &str,
) -> Option<String> {
    OhPackage::load_from_root(package_root)
        .and_then(|p| p.dependency_value(specifier).map(str::to_string))
}

pub(super) fn resolve_source_request(base_dir: &Path, request: &str) -> Option<PathBuf> {
    let base = base_dir.join(request);
    if base.is_file() {
        return Some(canonical_or_owned(&base));
    }
    for ext in ["ets", "ts", "js", "mjs", "cjs", "json", "json5"] {
        let candidate = base.with_extension(ext);
        if candidate.is_file() {
            return Some(canonical_or_owned(&candidate));
        }
    }
    for index in [
        "index.ets",
        "index.ts",
        "index.js",
        "index.mjs",
        "index.cjs",
        "index.json",
    ] {
        let candidate = base.join(index);
        if candidate.is_file() {
            return Some(canonical_or_owned(&candidate));
        }
    }
    None
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
    if v.is_empty() { "1.0.0".to_string() } else { v }
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
            slash_path(rel).replace(package_dir, "pkg_modules")
        ));
    }

    for (module_name, module_root) in &loader_json.module_path_map {
        let module_root = canonical_or_owned(Path::new(module_root));
        if let Ok(rel) = package_root.strip_prefix(module_root.join(package_dir)) {
            return Some(format!(
                "pkg_modules@{}/{}",
                module_name,
                slash_path(rel).replace(package_dir, "pkg_modules")
            ));
        }
    }

    None
}

fn record_relative_path(ts_rel: &str) -> String {
    let rel = ts_rel
        .trim_start_matches("src/main/")
        .trim_end_matches(".ets")
        .trim_end_matches(".ts")
        .trim_end_matches(".js");
    rel.to_string()
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn package_entry_path(package_root: &Path) -> Option<PathBuf> {
    let pkg = OhPackage::load_from_root(package_root);
    let request = pkg
        .as_ref()
        .and_then(|p| p.exports_import_field())
        .or_else(|| pkg.as_ref().and_then(|p| p.main_field().map(str::to_string)))
        .unwrap_or_else(|| "index".to_string());
    resolve_package_request(package_root, &request)
        .or_else(|| resolve_package_request(package_root, "index"))
}

fn resolve_package_request(package_root: &Path, request: &str) -> Option<PathBuf> {
    let request = request.trim_start_matches("./");
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
            let entry_path = slash_path(entry.path());
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
    use arkdown_core::{ArkChunk, ChunkGraph};
    use index_vec::IndexVec;
    use rustc_hash::FxHashMap;

    use super::compile_module_ids;

    #[test]
    fn compile_module_ids_prefers_full_rolldown_module_graph() {
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
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        };

        assert_eq!(
            compile_module_ids(&graph),
            vec![
                "/project/entry/src/main/ets/Index.ets".to_string(),
                "/project/entry/src/main/ets/PureReexport.ts".to_string(),
            ]
        );
    }
}
