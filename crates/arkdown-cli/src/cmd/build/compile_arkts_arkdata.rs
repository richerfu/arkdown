//! H13 — ArkData compile pass.
//!
//! Implements `arkDataCompile()` from hvigor: a separate compiler invocation
//! for `.ets` source files that define ArkData (cloud database) schemas.
//! These files use `@CloudData`, `@Field`, `@RelationTo` and similar ArkData
//! decorators from `@kit.ArkData` / `@ohos/cloudData`.
//!
//! The pass scans `src/main/ets/` for files that import from `@kit.ArkData`
//! or `@ohos/cloudData`, transforms them, and compiles to `ets/data.abc`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rustc_hash::FxHashMap;
use walkdir::WalkDir;

use arkdown_core::{ArkdownConfig, BuildMode, ModuleConfig, OhosModuleType};
use arkdown_plugin_ets_transform::{EtsTransformPlugin, ResourceReference};
use arkdown_profile::LoaderJson;
use arkdown_ts2abc::{compile_to_abc, Ts2abcInput, Ts2abcOptions};

use super::compile_arkts_modules::{
    compile_source_passthrough, rewrite_compile_module_imports_to_ohm,
};
use super::intermediates::{canonical_or_owned, path_string};
use super::ResolvedImportMap;

/// Import patterns that identify ArkData source files.
const ARKDATA_MARKERS: &[&str] = &[
    "@kit.ArkData",
    "@ohos/cloudData",
    "@ohos:data.cloudData",
    "relationalStore",
    "@ohos/distributedDataObject",
];

pub(super) fn compile_arkdata_pass(
    module: &ModuleConfig,
    loader_json: &LoaderJson,
    config: &Arc<ArkdownConfig>,
    resource_refs: &FxHashMap<String, ResourceReference>,
) -> Result<()> {
    // ArkData compile only applies to HAP/HSP modules.
    if matches!(module.module_type, OhosModuleType::Har) {
        return Ok(());
    }

    let ets_root = module.src_path.join("src/main/ets");
    if !ets_root.exists() {
        return Ok(());
    }

    // Collect .ets files that reference ArkData APIs.
    let arkdata_sources: Vec<PathBuf> = WalkDir::new(&ets_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map_or(false, |x| x == "ets")
        })
        .filter_map(|e| {
            let path = e.path().to_path_buf();
            let content = std::fs::read_to_string(&path).ok()?;
            if ARKDATA_MARKERS.iter().any(|m| content.contains(m)) {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if arkdata_sources.is_empty() {
        return Ok(());
    }

    tracing::debug!(
        count = arkdata_sources.len(),
        "H13: ArkData compile pass — compiling cloud-data schema files"
    );

    let mode_dir = match config.build_mode {
        BuildMode::Debug => "debug_arkdata",
        BuildMode::Release => "release_arkdata",
    };
    let cache_root = module
        .src_path
        .join("build/default/cache/default/default@CompileArkTS/esmodule")
        .join(mode_dir);
    std::fs::create_dir_all(&cache_root).ok();

    let transformer =
        EtsTransformPlugin::with_resource_references(config.clone(), resource_refs.clone());
    let resolved_imports: ResolvedImportMap = Default::default();
    let mut inputs: Vec<Ts2abcInput> = Vec::new();

    for source_path in &arkdata_sources {
        let source_path = canonical_or_owned(source_path);
        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("ets")
            .to_string();
        let rel = source_path
            .strip_prefix(&ets_root)
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or_else(|_| source_path.to_string_lossy().into_owned());
        let stem = rel
            .trim_end_matches(&format!(".{ext}"))
            .replace(['/', '\\'], "@");
        let cache_path = cache_root.join(format!("{}.ts", stem));
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let source = match std::fs::read_to_string(&source_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    path = %source_path.display(),
                    error = %e,
                    "H13: failed to read ArkData source"
                );
                continue;
            }
        };

        let transformed = transformer
            .do_transform(&source, &path_string(&source_path))
            .ok()
            .flatten();
        let code = transformed
            .as_ref()
            .and_then(|o| o.code.clone())
            .unwrap_or_else(|| compile_source_passthrough(&source, &ext));
        let code = rewrite_compile_module_imports_to_ohm(
            &code,
            &source_path,
            &cache_root,
            loader_json,
            config,
            &resolved_imports,
        );

        if let Err(e) = std::fs::write(&cache_path, &code) {
            tracing::warn!(error = %e, "H13: failed to write ArkData cache TS");
            continue;
        }

        let record_name = format!(
            "{}::ets/{}",
            loader_json.module_name,
            rel.trim_end_matches(&format!(".{ext}"))
        );
        inputs.push(Ts2abcInput {
            path: cache_path,
            record_name,
            source_file: Some(path_string(&source_path)),
            package_name: None,
            is_commonjs: false,
            source_map: None,
        });
    }

    if inputs.is_empty() {
        return Ok(());
    }

    let output_dir = PathBuf::from(&loader_json.ace_module_build);
    let data_abc_path = output_dir.join("data.abc");
    let abc_cache_dir = module.src_path.join("build/default/cache/abc_arkdata");
    std::fs::create_dir_all(&abc_cache_dir).ok();

    compile_to_abc(Ts2abcOptions {
        inputs,
        output: data_abc_path,
        target_api_version: config.sdk_config.compile_api_version,
        target_api_sub_version: config.product.compatible_sdk_version_stage.clone(),
        debug_info: matches!(config.build_mode, BuildMode::Debug),
        opt_level: if matches!(config.build_mode, BuildMode::Release) {
            2
        } else {
            0
        },
        merge_abc: true,
        module: true,
        commonjs: false,
        cache_file: Some(abc_cache_dir),
        npm_module_entry_list: None,
    })
    .map_err(|e| anyhow::anyhow!("H13: ArkData ABC compile failed: {}", e))?;

    Ok(())
}
