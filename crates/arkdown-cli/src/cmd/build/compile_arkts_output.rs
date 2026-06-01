use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context, Result};
pub(super) use arkdown::compile_entry::{compile_entry_chunk_name, compile_entry_stem_counts};
use arkdown::BundleOutput;
use arkdown_core::{
    hvigor_source_maps_json, ArkdownConfig, HvigorSourceMapEntry, HvigorSourceMapOptions,
    ModuleConfig,
};

use super::compile_arkts_handoff::read_compile_arkts_source_maps;

#[derive(Debug, Clone)]
pub(super) struct ChunkCompileMeta {
    pub(super) source_file: Option<String>,
}

pub(super) fn collect_unresolved_external_imports(bundle_out: &BundleOutput) -> Vec<String> {
    let mut imports = Vec::new();
    let internal_files: HashSet<String> = bundle_out
        .chunk_files
        .iter()
        .filter_map(|(_, path)| path.file_name()?.to_str().map(str::to_string))
        .collect();
    for chunk in &bundle_out.chunk_graph.chunks {
        for specifier in chunk.imports.iter().chain(chunk.dynamic_imports.iter()) {
            if internal_files.contains(specifier.trim_start_matches("./")) {
                continue;
            }
            if is_compile_unresolved_external(specifier) && !imports.contains(specifier) {
                imports.push(specifier.clone());
            }
        }
    }
    imports
}

fn is_compile_unresolved_external(specifier: &str) -> bool {
    !(specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with('/')
        || specifier.starts_with("@bundle:")
        || specifier.starts_with("@shared:")
        || specifier.starts_with("@ohos:")
        || specifier.starts_with("@ohos.")
        || specifier.starts_with("@kit.")
        || specifier.starts_with("@hms.")
        || specifier.starts_with("@system.")
        || specifier.starts_with("@native:")
        || specifier.ends_with(".so"))
}

pub(super) fn format_unresolved_import_errors(imports: &[String]) -> String {
    imports
        .iter()
        .enumerate()
        .map(|(idx, specifier)| {
            format!(
                "{} ERROR: 10505001 ArkTS Compiler Error\nError Message: Cannot find module '{}' or its corresponding type declarations.",
                idx + 1,
                specifier
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn write_hvigor_source_maps(
    bundle_out: &BundleOutput,
    module: &ModuleConfig,
    loader_json: &arkdown_profile::LoaderJson,
    config: &Arc<ArkdownConfig>,
    record_name_map: &HashMap<String, ChunkCompileMeta>,
) -> Result<()> {
    let Some(source_maps_path) = &bundle_out.source_maps_path else {
        return Ok(());
    };

    if let Some(cache_maps) = read_compile_arkts_source_maps(module, config)? {
        fs_err::write(source_maps_path, cache_maps)?;
        return Ok(());
    }

    let mut source_maps = Vec::new();
    for chunk in bundle_out.chunk_graph.chunks.iter() {
        let Some(meta) = record_name_map.get(chunk.name.as_str()) else {
            continue;
        };
        let Some(source_file) = meta.source_file.as_deref() else {
            continue;
        };
        let Some(raw_map) = chunk.source_map.as_deref() else {
            continue;
        };

        let map: serde_json::Value = serde_json::from_str(raw_map)
            .with_context(|| format!("failed to parse source map for chunk {}", chunk.name))?;
        let mappings = map
            .get("mappings")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        source_maps.push(HvigorSourceMapEntry {
            chunk_name: chunk.name.to_string(),
            mappings: mappings.to_string(),
            source_file: Some(source_file.to_string()),
            fallback_source_file: None,
        });
    }

    if !source_maps.is_empty() {
        let content = hvigor_source_maps_json(
            &source_maps,
            &HvigorSourceMapOptions {
                module_name: loader_json.module_name.clone(),
                package_name: loader_json.module_name.clone(),
                version: "1.0.0".to_string(),
                chunk_source_files: HashMap::new(),
            },
        )
        .context("failed to serialize hvigor sourceMaps.map")?;
        fs_err::write(source_maps_path, content)?;
    }

    Ok(())
}
