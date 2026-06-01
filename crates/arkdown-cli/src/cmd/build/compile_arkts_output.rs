use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use arkdown::BundleOutput;
use arkdown_core::{ArkdownConfig, ModuleConfig};

use super::compile_arkts_handoff::read_compile_arkts_source_maps;
use super::compile_arkts_modules::with_extension;

#[derive(Debug, Clone)]
pub(super) struct ChunkCompileMeta {
    pub(super) source_file: Option<String>,
}

pub(super) fn compile_entry_stem_counts(entries: &[String]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for entry in entries {
        if let Some(stem) = Path::new(entry.as_str())
            .file_stem()
            .and_then(|s| s.to_str())
        {
            *counts.entry(stem.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

pub(super) fn compile_entry_chunk_name(
    entry: &str,
    index: usize,
    stem_counts: &HashMap<String, usize>,
) -> String {
    let Some(stem) = Path::new(entry).file_stem().and_then(|s| s.to_str()) else {
        return format!("entry_{index}");
    };

    if stem_counts.get(stem).copied().unwrap_or(0) <= 1 {
        return stem.to_string();
    }

    let stripped = entry
        .trim_start_matches("./")
        .trim_end_matches(".ets")
        .trim_end_matches(".ts")
        .trim_end_matches(".js");
    let mut name = String::with_capacity(stripped.len());
    for ch in stripped.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
            name.push(ch);
        } else {
            name.push('_');
        }
    }
    if name.is_empty() {
        format!("entry_{index}")
    } else {
        name
    }
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

    let mut source_maps = serde_json::Map::new();
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
        let source_file = source_file.trim_start_matches("./");
        let key = format!(
            "{}|{}|1.0.0|src/main/{}",
            loader_json.module_name,
            loader_json.module_name,
            with_extension(source_file, "ts")
        );
        let file = Path::new(source_file)
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(source_file);
        let source = format!("{}/src/main/{}", loader_json.module_name, source_file);

        source_maps.insert(
            key,
            serde_json::json!({
                "version": 3,
                "file": file,
                "sourceRoot": "",
                "sources": [source],
                "names": [],
                "mappings": mappings,
                "entry-package-info": format!("{}|1.0.0", loader_json.module_name),
            }),
        );
    }

    if !source_maps.is_empty() {
        let content = serde_json::to_string_pretty(&source_maps)
            .context("failed to serialize hvigor sourceMaps.map")?;
        fs_err::write(source_maps_path, content)?;
    }

    Ok(())
}
