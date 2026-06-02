use super::compile_arkts_handoff::{write_compile_arkts_abc_handoff, CompileArkTsInfo};
use super::compile_arkts_modules::{
    compile_module_info, compile_source_passthrough, rewrite_compile_module_imports_to_ohm,
    slash_path,
};
use super::compile_arkts_package_entries::PackageEntryInfos;
use super::*;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, serde::Deserialize)]
struct FormConfigFile {
    #[serde(default)]
    forms: Vec<FormConfigEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct FormConfigEntry {
    src: Option<String>,
    #[serde(rename = "uiSyntax")]
    ui_syntax: Option<String>,
}

pub(super) fn compile_widget_pass(
    module: &ModuleConfig,
    manifest: &arkdown_core::HapManifest,
    loader_json: &arkdown_profile::LoaderJson,
    config: &Arc<ArkdownConfig>,
    resource_refs: &FxHashMap<String, ResourceReference>,
) -> Result<()> {
    if matches!(module.module_type, OhosModuleType::Har) {
        return Ok(());
    }

    let widget_sources = widget_sources(module, &manifest.extension_abilities)?;
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
    let mut compile_infos = Vec::new();
    let mut component_collection = serde_json::Map::new();

    for source_path in widget_sources {
        let source_path = canonical_or_owned(&source_path);
        let Some(entry_info) = compile_module_info(&source_path, &cache_root, loader_json, config)
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
        let code = rewrite_compile_module_imports_to_ohm(
            &code,
            &entry_info.source_path,
            &cache_root,
            loader_json,
            config,
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
        let collection_key = entry_info
            .source_path
            .strip_prefix(&config.project_root)
            .ok()
            .map(slash_path)
            .unwrap_or_else(|| slash_path(&entry_info.source_path));
        component_collection.insert(collection_key, serde_json::json!(components));

        compile_infos.push(CompileArkTsInfo {
            ts_path: entry_info.ts_path.clone(),
            record_name: entry_info.record_name.clone(),
            module_type: entry_info.module_type.clone(),
            source_file: entry_info.source_file_info.clone(),
            package_name: entry_info.package_name.clone(),
            origin_source_lang: entry_info.origin_source_lang.clone(),
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
    let handoff = write_compile_arkts_abc_handoff(&cache_root, &compile_infos, &pkg_entries)?;

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
    let widget_cache_file = module
        .src_path
        .join("build/default/cache/abc/widgets.abc.cache");
    if let Some(parent) = widget_cache_file.parent() {
        fs_err::create_dir_all(parent)?;
    }
    compile_to_abc(Ts2abcOptions {
        inputs: handoff.inputs,
        output: widget_abc,
        target_api_version: config.sdk_config.compile_api_version,
        target_api_sub_version: None,
        debug_info: matches!(config.build_mode, BuildMode::Debug),
        opt_level: if matches!(config.build_mode, BuildMode::Release) {
            2
        } else {
            0
        },
        merge_abc: true,
        module: true,
        commonjs: false,
        cache_file: Some(widget_cache_file),
        npm_module_entry_list: handoff.npm_entries_path,
    })
    .map_err(|e| anyhow::anyhow!("widget ABC compile failed: {}", e))?;

    Ok(())
}

fn widget_sources(
    module: &ModuleConfig,
    extension_abilities: &[ExtensionAbilityEntry],
) -> Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    let mut seen = FxHashSet::default();
    for ability in extension_abilities {
        let extension_type = ability.extension_type.to_ascii_lowercase();
        if extension_type != "form" && extension_type != "formextension" {
            continue;
        }
        let profile_names = form_profile_names(ability);
        for profile_name in profile_names {
            let profile_path = module
                .src_path
                .join("src/main/resources/base/profile")
                .join(format!("{profile_name}.json"));
            if !profile_path.exists() {
                continue;
            }
            let content = fs_err::read_to_string(&profile_path)?;
            let form_config: FormConfigFile = json5::from_str(&content)
                .with_context(|| format!("failed to parse {}", profile_path.display()))?;
            for form in form_config.forms {
                if let Some(ui_syntax) = &form.ui_syntax {
                    if ui_syntax != "arkts" {
                        continue;
                    }
                }
                let Some(src) = form.src else {
                    continue;
                };
                let source_path = module
                    .src_path
                    .join("src/main")
                    .join(src.trim_start_matches("./"));
                if seen.insert(source_path.clone()) {
                    sources.push(source_path);
                }
            }
        }
    }
    sources.sort();
    Ok(sources)
}

fn form_profile_names(ability: &ExtensionAbilityEntry) -> Vec<String> {
    let names: Vec<String> = ability
        .metadata
        .iter()
        .filter_map(|metadata| metadata.resource.as_deref())
        .filter_map(|resource| resource.strip_prefix("$profile:"))
        .map(str::to_string)
        .collect();
    if names.is_empty() {
        vec!["form_config".to_string()]
    } else {
        names
    }
}
