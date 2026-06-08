use super::compile_arkts_abc::{AbcCompilePlan, AbcCompileRequest};
use super::compile_arkts_handoff::{write_compile_arkts_abc_handoff, CompileArkTsInfo};
use super::compile_arkts_modules::{
    compile_source_passthrough, rewrite_compile_module_imports_to_ohm, CompileModuleImportResolver,
};
use super::compile_arkts_package_entries::PackageEntryInfos;
use super::*;
use arkdown_config::build_profile::parse_api_version;
use arkdown_profile::{collect_module_form_sources, ModuleFormProfileInput};
use rustc_hash::{FxHashMap, FxHashSet};

pub(super) fn compile_widget_pass(
    module: &ModuleConfig,
    loader_json: &arkdown_profile::LoaderJson,
    config: &Arc<ArkdownConfig>,
    resource_refs: &FxHashMap<String, ResourceReference>,
    abc_compile_plan: &AbcCompilePlan,
    widget_sources: &[PathBuf],
    disable_abc_cache: bool,
) -> Result<()> {
    if matches!(module.module_type, OhosModuleType::Har) {
        return Ok(());
    }

    if widget_sources.is_empty() {
        return Ok(());
    }

    let mode_dir = match config.build_mode {
        BuildMode::Debug => "debug_widget",
        BuildMode::Release => "release_widget",
    };
    let cache_root = module
        .src_path
        .join("build/default/cache/default/default@CompileArkTS/esmodule")
        .join(mode_dir);
    let transformer =
        EtsTransformPlugin::with_resource_references(config.clone(), resource_refs.clone());
    let resolved_imports = Default::default();
    let mut compile_infos = Vec::new();
    let mut component_collection = serde_json::Map::new();

    for source_path in widget_sources {
        let source_path = canonical_or_owned(&source_path);
        let Some(entry_info) =
            CompileModuleInfo::compute(&source_path, &cache_root, loader_json, config)
        else {
            continue;
        };
        let source = fs_err::read_to_string(&entry_info.source_path).with_context(|| {
            format!(
                "failed to read widget source {}",
                entry_info.source_path.display()
            )
        })?;
        let transformed = transformer
            .do_transform(&source, &path_string(&entry_info.source_path))
            .with_context(|| {
                format!(
                    "failed to transform widget source {}",
                    entry_info.source_path.display()
                )
            })?;
        let code = transformed
            .as_ref()
            .and_then(|output| output.code.clone())
            .unwrap_or_else(|| compile_source_passthrough(&source, &entry_info.source_ext));
        let import_resolver = CompileModuleImportResolver::new(
            &cache_root,
            loader_json,
            config,
            &resolved_imports,
            None,
        );
        let code = rewrite_compile_module_imports_to_ohm(
            &code,
            &entry_info.ts_path,
            &entry_info.source_path,
            &import_resolver,
        );
        if let Some(parent) = entry_info.ts_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(&entry_info.ts_path, code).with_context(|| {
            format!("failed to write widget TS {}", entry_info.ts_path.display())
        })?;

        let components =
            collect_arkui_component_names(&source, &path_string(&entry_info.source_path))
                .with_context(|| {
                    format!(
                        "failed to collect widget components from {}",
                        entry_info.source_path.display()
                    )
                })?;
        let collection_key = widget_component_collection_key(
            &entry_info.source_path,
            &canonical_or_owned(&config.project_root),
        );
        component_collection.insert(collection_key, serde_json::json!(components));

        compile_infos.push(CompileArkTsInfo {
            ts_path: entry_info.ts_path.clone(),
            record_name: entry_info.record_name.clone(),
            module_type: entry_info.module_type.clone(),
            source_file: entry_info.source_file_info.clone(),
            package_name: entry_info.package_name.clone(),
            origin_source_lang: entry_info.origin_source_lang.clone(),
            is_shared_module: entry_info.is_shared_module,
            abc_cache_path: None,
        });
    }

    if compile_infos.is_empty() {
        return Ok(());
    }

    compile_infos.sort_by(|a, b| a.source_file.cmp(&b.source_file));
    let mut pkg_entries = PackageEntryInfos::default();
    if !loader_json.use_normalized_ohm_url {
        pkg_entries.add_dynamic_imports(loader_json, config);
    }
    pkg_entries.add_native_entries();
    let mut handoff = write_compile_arkts_abc_handoff(&cache_root, &compile_infos, &pkg_entries)?;
    if disable_abc_cache {
        handoff = handoff.without_program_cache();
    }

    let collection_value = serde_json::Value::Object(component_collection);
    let collection_bytes = serde_json::to_vec_pretty(&collection_value)?;
    write_bytes(
        &cache_root.join("component_collection.json"),
        &collection_bytes,
    )?;
    write_bytes(
        &PathBuf::from(&loader_json.ace_module_build)
            .join("widget")
            .join("component_collection.json"),
        &collection_bytes,
    )?;
    write_json_pretty(
        &module
            .src_path
            .join("build/default/intermediates/loader/default/widget_loader.json"),
        &serde_json::to_value(loader_json)?,
    )?;

    let widget_abc = PathBuf::from(&loader_json.ace_module_build).join("widgets.abc");
    // Widget cache uses its own handoff modules.cache, matching
    // es2abc --cache-file @... without reusing the main module manifest.
    let modules_cache_path = handoff.modules_cache_path.clone();
    abc_compile_plan
        .compile(AbcCompileRequest {
            inputs: handoff.inputs,
            output: widget_abc,
            input_file_list: handoff.files_info_path,
            cache_file: modules_cache_path,
            npm_module_entry_list: handoff.npm_entries_path,
        })
        .map_err(|e| anyhow::anyhow!("widget ABC compile failed: {}", e))?;

    Ok(())
}

pub(super) fn collect_widget_sources(
    config: &ArkdownConfig,
    module: &ModuleConfig,
    manifest: &arkdown_core::HapManifest,
) -> Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    let mut seen = FxHashSet::default();
    let module_json_path = module.src_path.join("src/main/module.json5");
    let form_sources = collect_module_form_sources(ModuleFormProfileInput {
        module_name: &module.name,
        module_root: &module.src_path,
        module_type: &module.module_type,
        module_json_path: &module_json_path,
        extension_abilities: &manifest.extension_abilities,
        project_modules: &config.modules,
        compatible_api_version: parse_api_version(&config.product.compatible_sdk_version)
            .or(Some(config.sdk_config.compile_api_version)),
    })?;
    for source in form_sources {
        if !source.is_arkts() {
            continue;
        }
        if seen.insert(source.source_path.clone()) {
            sources.push(source.source_path);
        }
    }
    sources.sort();
    Ok(sources)
}

fn widget_component_collection_key(source_path: &Path, project_root: &Path) -> String {
    for source_root in ["products", "features", "commons"] {
        if let Some(rel) = path_from_named_component(source_path, source_root) {
            return arkdown_path::to_slash(&rel);
        }
    }

    source_path
        .strip_prefix(project_root)
        .map(arkdown_path::to_slash)
        .unwrap_or_else(|_| arkdown_path::to_slash(source_path))
}

fn path_from_named_component(path: &Path, component_name: &str) -> Option<PathBuf> {
    let mut rel = PathBuf::new();
    let mut found = false;
    for component in path.components() {
        let component_os = component.as_os_str();
        if found {
            rel.push(component_os);
        } else if component_os == std::ffi::OsStr::new(component_name) {
            found = true;
            rel.push(component_os);
        }
    }
    found.then_some(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_collection_key_prefers_hvigor_source_root_components() {
        let source = Path::new(
            "/private/tmp/arkdown-hmosworld/products/phone/src/main/ets/widget/pages/WidgetCard.ets",
        );
        let project_root = Path::new("/tmp/arkdown-hmosworld");

        assert_eq!(
            widget_component_collection_key(source, project_root),
            "products/phone/src/main/ets/widget/pages/WidgetCard.ets"
        );
    }

    #[test]
    fn widget_collection_key_falls_back_to_project_relative_path() {
        let project_root = Path::new("/workspace/app");
        let source = Path::new("/workspace/app/entry/src/main/ets/widget/pages/Card.ets");

        assert_eq!(
            widget_component_collection_key(source, project_root),
            "entry/src/main/ets/widget/pages/Card.ets"
        );
    }
}
