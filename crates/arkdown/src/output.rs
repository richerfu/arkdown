use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

use arkdown_core::{
    hvigor_source_maps_json, ArkdownError, ChunkGraph, HvigorSourceMapEntry, HvigorSourceMapOptions,
};

pub const MODULES_ABC_FILE_NAME: &str = "modules.abc";
pub const SOURCE_MAPS_FILE_NAME: &str = "sourceMaps.map";

/// Files written by the ArkTS bundler stage.
#[derive(Debug)]
pub struct WrittenOutput {
    /// `(chunk_name, absolute_path)` for each written `.js` file.
    pub chunk_files: Vec<(String, PathBuf)>,
    /// `sourceMaps.map` path if a real source map was produced.
    pub source_maps_path: Option<PathBuf>,
}

/// Module-level metadata needed to emit the hvigor per-file `sourceMaps.map` format.
///
/// The golden format uses keys of the form
/// `{module_name}|{package_name}|{version}|src/main/{path}.ts`
/// and each entry carries an `"entry-package-info"` field.
#[derive(Debug, Clone)]
pub struct SourceMapOptions {
    /// OHOS module name (e.g. `"entry"`).
    pub module_name: String,
    /// Package name from `oh-package.json5` (typically equals `module_name`).
    pub package_name: String,
    /// Package version string (e.g. `"1.0.0"`).
    pub version: String,
    /// Map from chunk stem (e.g. `"EntryAbility"`) to the compile-entry source
    /// path relative to `src/main` (e.g. `"ets/entryability/EntryAbility.ets"`).
    /// Used to build the per-file key and the `sources` / `file` fields.
    pub chunk_source_files: FxHashMap<String, String>,
}

impl From<&SourceMapOptions> for HvigorSourceMapOptions {
    fn from(value: &SourceMapOptions) -> Self {
        Self {
            module_name: value.module_name.clone(),
            package_name: value.package_name.clone(),
            version: value.version.clone(),
            chunk_source_files: value.chunk_source_files.clone(),
        }
    }
}

/// Path used by native ABC compilation and packaging for the merged module ABC.
pub fn modules_abc_path(loader_out_ets_dir: &Path) -> PathBuf {
    loader_out_ets_dir.join(MODULES_ABC_FILE_NAME)
}

/// Path used by native ABC compilation and packaging for debug source maps.
pub fn source_maps_path(loader_out_ets_dir: &Path) -> PathBuf {
    loader_out_ets_dir.join(SOURCE_MAPS_FILE_NAME)
}

/// Write every chunk in `chunk_graph` to `output_dir` as `<name>.js`.
///
/// Creates the output directory (and any missing parents) if it does not exist.
/// Returns a list of `(chunk_name, absolute_output_path)` pairs.
pub fn write_chunks(
    chunk_graph: &ChunkGraph,
    output_dir: &Path,
) -> Result<Vec<(String, PathBuf)>, ArkdownError> {
    Ok(write_output(chunk_graph, output_dir, None)?.chunk_files)
}

/// Write JavaScript chunks and the hvigor-compatible source map artifact.
///
/// The native CompileArkTS contract keeps all loader outputs under
/// `loader_out/default/ets`: JavaScript chunks are direct Rust ts2abc inputs,
/// `modules.abc` is written by the ABC backend, and `sourceMaps.map` is emitted
/// only when the bundler produced a real source map from compile inputs.
///
/// When `source_map_opts` is `Some`, the emitted `sourceMaps.map` uses the
/// hvigor per-file dict format (keyed by `{module}|{pkg}|{ver}|src/main/{path}.ts`)
/// that matches the golden reference.  When `None`, the raw rolldown source map
/// is written as-is (single chunk) or omitted (multiple chunks without metadata).
pub fn write_output(
    chunk_graph: &ChunkGraph,
    output_dir: &Path,
    source_map_opts: Option<&SourceMapOptions>,
) -> Result<WrittenOutput, ArkdownError> {
    fs_err::create_dir_all(output_dir).map_err(|e| ArkdownError::Io(e.into()))?;

    let mut files = Vec::new();
    let mut source_maps = Vec::new();

    for chunk in chunk_graph.chunks.iter() {
        let filename = format!("{}.js", chunk.name);
        let path = output_dir.join(&filename);
        fs_err::write(&path, &chunk.code).map_err(|e| ArkdownError::Io(e.into()))?;
        files.push((chunk.name.clone(), path));

        if let Some(ref map) = chunk.source_map {
            let value = validate_real_source_map(map)?;
            source_maps.push(ChunkSourceMap {
                chunk_name: chunk.name.clone(),
                generated_line_count: chunk.code.lines().count(),
                raw_map: map.as_str(),
                value,
            });
        }
    }

    let source_maps_path = if !source_maps.is_empty() {
        let path = source_maps_path(output_dir);
        let content = if let Some(opts) = source_map_opts {
            hvigor_source_maps_content(&source_maps, opts)?
        } else {
            source_maps_map_content(&source_maps)?
        };
        fs_err::write(&path, content).map_err(|e| ArkdownError::Io(e.into()))?;
        Some(path)
    } else {
        None
    };

    Ok(WrittenOutput {
        chunk_files: files,
        source_maps_path,
    })
}

struct ChunkSourceMap<'a> {
    chunk_name: String,
    generated_line_count: usize,
    raw_map: &'a str,
    value: serde_json::Value,
}

fn validate_real_source_map(map: &str) -> Result<serde_json::Value, ArkdownError> {
    let value: serde_json::Value = serde_json::from_str(map).map_err(|e| {
        ArkdownError::Bundler(format!(
            "source map is not valid JSON; keep sourceMaps.map unwritten until real map wiring is available: {e}"
        ))
    })?;
    let Some(object) = value.as_object() else {
        return Err(ArkdownError::Bundler(
            "source map must be a JSON object generated from compile inputs".to_string(),
        ));
    };
    if object.is_empty() {
        return Err(ArkdownError::Bundler(
            "refusing to emit fake empty sourceMaps.map".to_string(),
        ));
    }

    let has_sources = value
        .get("sources")
        .and_then(|sources| sources.as_array())
        .is_some_and(|sources| !sources.is_empty());
    let has_mappings = value
        .get("mappings")
        .and_then(|mappings| mappings.as_str())
        .is_some_and(|mappings| !mappings.is_empty());
    if !has_sources && !has_mappings {
        return Err(ArkdownError::Bundler(
            "refusing to emit sourceMaps.map without compile input sources or mappings".to_string(),
        ));
    }

    Ok(value)
}

fn source_maps_map_content(source_maps: &[ChunkSourceMap<'_>]) -> Result<String, ArkdownError> {
    if let [source_map] = source_maps {
        return Ok(source_map.raw_map.to_string());
    }

    let mut next_line_offset = 0usize;
    let sections = source_maps
        .iter()
        .map(|source_map| {
            let section = serde_json::json!({
                "offset": {
                    "line": next_line_offset,
                    "column": 0,
                },
                "map": source_map.value,
            });
            next_line_offset += source_map.generated_line_count + 1;
            section
        })
        .collect::<Vec<_>>();
    let chunk_names = source_maps
        .iter()
        .map(|source_map| source_map.chunk_name.as_str())
        .collect::<Vec<_>>();

    serde_json::to_string(&serde_json::json!({
        "version": 3,
        "file": MODULES_ABC_FILE_NAME,
        "sections": sections,
        "x_arkdown_chunks": chunk_names,
    }))
    .map_err(|e| ArkdownError::Bundler(format!("failed to serialize sourceMaps.map: {e}")))
}

/// Emit the hvigor per-file dict `sourceMaps.map` format.
///
/// The output is a JSON object whose keys are
/// `{module_name}|{package_name}|{version}|src/main/{path}.ts`
/// and whose values are individual source map objects with an extra
/// `"entry-package-info"` field, matching the hvigor golden reference.
fn hvigor_source_maps_content(
    source_maps: &[ChunkSourceMap<'_>],
    opts: &SourceMapOptions,
) -> Result<String, ArkdownError> {
    let entries = source_maps
        .iter()
        .map(|source_map| {
            let mappings = source_map
                .value
                .get("mappings")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let source_map_source_file = source_map
                .value
                .get("sources")
                .and_then(|sources| sources.as_array())
                .and_then(|sources| sources.first())
                .and_then(|source| source.as_str())
                .map(str::to_string);
            HvigorSourceMapEntry {
                chunk_name: source_map.chunk_name.clone(),
                mappings: mappings.to_string(),
                source_file: None,
                source_map_source_file,
            }
        })
        .collect::<Vec<_>>();
    hvigor_source_maps_json(&entries, &HvigorSourceMapOptions::from(opts))
        .map_err(|e| ArkdownError::Bundler(format!("failed to serialize sourceMaps.map: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::ArkChunk;
    use index_vec::IndexVec;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_output_dir(test_name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("arkdown-{test_name}-{nonce}"))
    }

    fn graph(chunks: Vec<ArkChunk>) -> ChunkGraph {
        ChunkGraph {
            chunks: chunks.into_iter().collect::<IndexVec<_, _>>(),
            full_module_ids: Vec::new(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        }
    }

    #[test]
    fn layout_helpers_point_to_loader_out_ets_artifacts() {
        let loader_out = Path::new("build/default/intermediates/loader_out/default/ets");

        assert_eq!(modules_abc_path(loader_out), loader_out.join("modules.abc"));
        assert_eq!(
            source_maps_path(loader_out),
            loader_out.join("sourceMaps.map")
        );
    }

    #[test]
    fn writes_source_maps_map_from_real_chunk_source_map() {
        let output_dir = temp_output_dir("source-map");
        let source_map =
            r#"{"version":3,"sources":["src/main/ets/pages/Index.ets"],"mappings":"AAAA"}"#;
        let chunk_graph = graph(vec![ArkChunk {
            name: "Index".to_string(),
            code: "console.log('ok');".to_string(),
            source_map: Some(source_map.to_string()),
            ..ArkChunk::default()
        }]);

        let written = write_output(&chunk_graph, &output_dir, None).expect("write output");

        let source_maps = output_dir.join("sourceMaps.map");
        assert_eq!(written.source_maps_path, Some(source_maps.clone()));
        assert_eq!(
            fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
            source_map
        );
        assert!(
            !output_dir.join("Index.js.map").exists(),
            "per-chunk source maps must not be written"
        );

        let _ = fs_err::remove_dir_all(output_dir);
    }

    #[test]
    fn writes_index_source_map_for_multiple_chunk_source_maps() {
        // Without SourceMapOptions, multi-chunk falls back to the index-sections format.
        let output_dir = temp_output_dir("multi-source-map");
        let chunk_graph = graph(vec![
            ArkChunk {
                name: "EntryAbility".to_string(),
                code: "console.log('entry');\n".to_string(),
                source_map: Some(
                    r#"{"version":3,"sources":["src/main/ets/entryability/EntryAbility.ets"],"mappings":"AAAA"}"#
                        .to_string(),
                ),
                ..ArkChunk::default()
            },
            ArkChunk {
                name: "Index".to_string(),
                code: "console.log('index');\n".to_string(),
                source_map: Some(
                    r#"{"version":3,"sources":["src/main/ets/pages/Index.ets"],"mappings":"BBBB"}"#
                        .to_string(),
                ),
                ..ArkChunk::default()
            },
        ]);

        let written = write_output(&chunk_graph, &output_dir, None).expect("write output");

        let source_maps = written.source_maps_path.expect("sourceMaps.map path");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");
        let sections = value
            .get("sections")
            .and_then(|sections| sections.as_array())
            .expect("sourceMaps.map sections");
        assert_eq!(sections.len(), 2);
        assert_eq!(
            sections[0]["map"]["sources"][0],
            "src/main/ets/entryability/EntryAbility.ets"
        );
        assert_eq!(
            sections[1]["map"]["sources"][0],
            "src/main/ets/pages/Index.ets"
        );

        let _ = fs_err::remove_dir_all(output_dir);
    }

    #[test]
    fn writes_hvigor_per_file_source_maps_with_options() {
        let output_dir = temp_output_dir("hvigor-source-map");
        let chunk_graph = graph(vec![
            ArkChunk {
                name: "EntryAbility".to_string(),
                code: "console.log('entry');\n".to_string(),
                source_map: Some(
                    r#"{"version":3,"sources":["src/main/ets/entryability/EntryAbility.ets"],"mappings":"AAAA"}"#
                        .to_string(),
                ),
                ..ArkChunk::default()
            },
            ArkChunk {
                name: "Index".to_string(),
                code: "console.log('index');\n".to_string(),
                source_map: Some(
                    r#"{"version":3,"sources":["src/main/ets/pages/Index.ets"],"mappings":"BBBB"}"#
                        .to_string(),
                ),
                ..ArkChunk::default()
            },
        ]);
        let mut chunk_source_files = FxHashMap::default();
        chunk_source_files.insert(
            "EntryAbility".to_string(),
            "ets/entryability/EntryAbility.ets".to_string(),
        );
        chunk_source_files.insert("Index".to_string(), "ets/pages/Index.ets".to_string());
        let opts = SourceMapOptions {
            module_name: "entry".to_string(),
            package_name: "entry".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files,
        };

        let written = write_output(&chunk_graph, &output_dir, Some(&opts)).expect("write output");

        let source_maps = written.source_maps_path.expect("sourceMaps.map path");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");

        // Must be a per-file dict, not a sections index.
        assert!(
            value.get("sections").is_none(),
            "hvigor format must not use sections"
        );
        let entry_key = "entry|entry|1.0.0|src/main/ets/entryability/EntryAbility.ts";
        let index_key = "entry|entry|1.0.0|src/main/ets/pages/Index.ts";
        assert!(value.get(entry_key).is_some(), "missing key {entry_key}");
        assert!(value.get(index_key).is_some(), "missing key {index_key}");

        let entry_map = &value[entry_key];
        assert_eq!(entry_map["version"], 3);
        assert_eq!(entry_map["file"], "EntryAbility.ets");
        assert_eq!(entry_map["sourceRoot"], "");
        assert_eq!(
            entry_map["sources"][0],
            "entry/src/main/ets/entryability/EntryAbility.ets"
        );
        assert_eq!(entry_map["names"], serde_json::json!([]));
        assert_eq!(entry_map["entry-package-info"], "entry|1.0.0");

        let _ = fs_err::remove_dir_all(output_dir);
    }

    #[test]
    fn rejects_fake_empty_source_map() {
        let output_dir = temp_output_dir("fake-source-map");
        let chunk_graph = graph(vec![ArkChunk {
            name: "Index".to_string(),
            code: "console.log('ok');".to_string(),
            source_map: Some("{}".to_string()),
            ..ArkChunk::default()
        }]);

        let err =
            write_output(&chunk_graph, &output_dir, None).expect_err("empty source map rejected");
        assert!(err.to_string().contains("fake empty sourceMaps.map"));

        let _ = fs_err::remove_dir_all(output_dir);
    }
}
