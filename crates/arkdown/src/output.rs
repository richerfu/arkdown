use rustc_hash::FxHashMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownError, ChunkGraph, HvigorSourceMapEntry, HvigorSourceMapOptions};
use arkdown_path::ArtifactPathSerializer;
use rolldown_sourcemap::{OwnedSourceMap as RolldownOwnedSourceMap, SourceMapBuilder};

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

impl WrittenOutput {
    /// Write JavaScript chunks from `chunk_graph` to `output_dir`.
    ///
    /// Returns a `WrittenOutput` with chunk file paths and optional source map path.
    pub fn write(
        chunk_graph: &ChunkGraph,
        output_dir: &Path,
        source_map_opts: Option<&SourceMapOptions>,
    ) -> Result<Self, ArkdownError> {
        Self::write_output(chunk_graph, output_dir, source_map_opts)
    }

    /// Write chunks without source map metadata; returns just the file list.
    pub fn write_chunks_only(
        chunk_graph: &ChunkGraph,
        output_dir: &Path,
    ) -> Result<Vec<(String, PathBuf)>, ArkdownError> {
        Ok(Self::write(chunk_graph, output_dir, None)?.chunk_files)
    }

    /// Path for the merged module ABC file within `loader_out_ets_dir`.
    pub fn abc_path_in(loader_out_ets_dir: &Path) -> PathBuf {
        loader_out_ets_dir.join(MODULES_ABC_FILE_NAME)
    }

    /// Path for the hvigor-format source map file within `loader_out_ets_dir`.
    pub fn source_maps_path_in(loader_out_ets_dir: &Path) -> PathBuf {
        loader_out_ets_dir.join(SOURCE_MAPS_FILE_NAME)
    }

    pub fn write_hvigor_source_maps_from_raw<'a, I>(
        output_dir: &Path,
        source_maps: I,
        source_map_opts: &SourceMapOptions,
    ) -> Result<Option<PathBuf>, ArkdownError>
    where
        I: IntoIterator<Item = (&'a str, &'a str, bool)>,
    {
        let source_maps = source_maps
            .into_iter()
            .map(|(chunk_name, raw_map, emits_javascript)| ChunkSourceMap {
                chunk_name,
                generated_line_count: 0,
                raw_map,
                emits_javascript,
            })
            .collect::<Vec<_>>();
        if source_maps.is_empty() {
            return Ok(None);
        }
        fs_err::create_dir_all(output_dir).map_err(ArkdownError::Io)?;
        let path = Self::source_maps_path(output_dir);
        Self::write_hvigor_source_maps(&path, &source_maps, source_map_opts)?;
        Ok(Some(path))
    }
}

/// Module-level metadata needed to emit the hvigor per-file `sourceMaps.map` format.
///
/// The golden format uses keys of the form
/// `{module_name}|{package_name}|{version}|src/main/{path}.ts`
/// and each entry carries an `"entry-package-info"` field.
#[derive(Debug, Clone)]
pub struct SourceMapOptions {
    /// Canonical project root used by latest SourceMapGenerator module paths.
    pub project_root: PathBuf,
    /// Canonical owning module root used for generated sources outside src/main.
    pub module_root: PathBuf,
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
    /// Package versions keyed by package name. OHPM store directory names are
    /// opaque, so source maps must not infer versions from `.ohpm` folders.
    pub package_versions: FxHashMap<String, String>,
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

struct ChunkSourceMap<'a> {
    chunk_name: &'a str,
    generated_line_count: usize,
    raw_map: &'a str,
    emits_javascript: bool,
}

struct IndentedJsonValueWriter<'a, W> {
    writer: &'a mut W,
    indent: &'static [u8],
}

impl<'a, W> IndentedJsonValueWriter<'a, W> {
    fn new(writer: &'a mut W, indent: &'static [u8]) -> Self {
        Self { writer, indent }
    }
}

impl<W: Write> Write for IndentedJsonValueWriter<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let mut consumed = 0;
        for segment in buffer.split_inclusive(|byte| *byte == b'\n') {
            self.writer.write_all(segment)?;
            consumed += segment.len();
            if segment.last() == Some(&b'\n') {
                self.writer.write_all(self.indent)?;
            }
        }
        Ok(consumed)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl WrittenOutput {
    fn source_maps_path(loader_out_ets_dir: &Path) -> PathBuf {
        loader_out_ets_dir.join(SOURCE_MAPS_FILE_NAME)
    }

    fn write_output(
        chunk_graph: &ChunkGraph,
        output_dir: &Path,
        source_map_opts: Option<&SourceMapOptions>,
    ) -> Result<WrittenOutput, ArkdownError> {
        fs_err::create_dir_all(output_dir).map_err(ArkdownError::Io)?;

        let mut files = Vec::new();
        let mut source_maps = Vec::new();

        for chunk in chunk_graph.chunks.iter() {
            let filename = ChunkOutputFileName::new(&chunk.name).js_file_name();
            let path = output_dir.join(&filename);
            if let Some(parent) = path.parent() {
                fs_err::create_dir_all(parent).map_err(ArkdownError::Io)?;
            }
            fs_err::write(&path, &chunk.code).map_err(ArkdownError::Io)?;
            files.push((chunk.name.clone(), path));

            if let Some(ref map) = chunk.source_map {
                source_maps.push(ChunkSourceMap {
                    chunk_name: chunk.name.as_str(),
                    generated_line_count: chunk.code.lines().count(),
                    raw_map: map.as_str(),
                    emits_javascript: false,
                });
            }
        }

        let source_maps_path = if !source_maps.is_empty() {
            let path = Self::source_maps_path(output_dir);
            if let Some(opts) = source_map_opts {
                Self::write_hvigor_source_maps(&path, &source_maps, opts)?;
            } else {
                let content = Self::source_maps_map_content(&source_maps)?;
                fs_err::write(&path, content).map_err(ArkdownError::Io)?;
            }
            Some(path)
        } else {
            None
        };

        Ok(WrittenOutput {
            chunk_files: files,
            source_maps_path,
        })
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
                "refusing to emit sourceMaps.map without compile input sources or mappings"
                    .to_string(),
            ));
        }

        Ok(value)
    }

    fn source_maps_map_content(source_maps: &[ChunkSourceMap<'_>]) -> Result<String, ArkdownError> {
        if let [source_map] = source_maps {
            Self::validate_real_source_map(source_map.raw_map)?;
            return Ok(source_map.raw_map.to_string());
        }

        let mut next_line_offset = 0usize;
        let sections = source_maps
            .iter()
            .map(|source_map| {
                let value = Self::validate_real_source_map(source_map.raw_map)?;
                let section = serde_json::json!({
                    "offset": {
                        "line": next_line_offset,
                        "column": 0,
                    },
                    "map": value,
                });
                next_line_offset += source_map.generated_line_count + 1;
                Ok(section)
            })
            .collect::<Result<Vec<_>, ArkdownError>>()?;
        let chunk_names = source_maps
            .iter()
            .map(|source_map| source_map.chunk_name)
            .collect::<Vec<_>>();

        serde_json::to_string(&serde_json::json!({
            "version": 3,
            "file": MODULES_ABC_FILE_NAME,
            "sections": sections,
            "x_arkdown_chunks": chunk_names,
        }))
        .map_err(|e| ArkdownError::Bundler(format!("failed to serialize sourceMaps.map: {e}")))
    }

    fn write_hvigor_source_maps(
        path: &Path,
        source_maps: &[ChunkSourceMap<'_>],
        opts: &SourceMapOptions,
    ) -> Result<(), ArkdownError> {
        let file = fs_err::File::create(path).map_err(ArkdownError::Io)?;
        let mut writer = BufWriter::with_capacity(1024 * 1024, file);
        let hvigor_opts = HvigorSourceMapOptions::from(opts);
        writer.write_all(b"{").map_err(ArkdownError::Io)?;
        let mut first = true;
        for source_map in source_maps {
            for entry in Self::hvigor_source_map_entries(source_map, opts)? {
                if first {
                    writer.write_all(b"\n").map_err(ArkdownError::Io)?;
                    first = false;
                } else {
                    writer.write_all(b",\n").map_err(ArkdownError::Io)?;
                }
                writer.write_all(b"  ").map_err(ArkdownError::Io)?;
                let (key, value) = hvigor_opts.entry_json(&entry);
                serde_json::to_writer(&mut writer, &key).map_err(|e| {
                    ArkdownError::Bundler(format!("failed to serialize sourceMaps.map: {e}"))
                })?;
                writer.write_all(b": ").map_err(ArkdownError::Io)?;
                let mut value_writer = IndentedJsonValueWriter::new(&mut writer, b"  ");
                serde_json::to_writer_pretty(&mut value_writer, &value).map_err(|e| {
                    ArkdownError::Bundler(format!("failed to serialize sourceMaps.map: {e}"))
                })?;
            }
        }
        if !first {
            writer.write_all(b"\n").map_err(ArkdownError::Io)?;
        }
        writer.write_all(b"}").map_err(ArkdownError::Io)?;
        writer.flush().map_err(ArkdownError::Io)
    }

    fn hvigor_source_map_entries(
        source_map: &ChunkSourceMap<'_>,
        opts: &SourceMapOptions,
    ) -> Result<Vec<HvigorSourceMapEntry>, ArkdownError> {
        let Ok(decoded) = RolldownOwnedSourceMap::from_json_string(source_map.raw_map) else {
            return Self::fallback_hvigor_source_map_entries(source_map);
        };

        let sources = decoded
            .get_sources()
            .enumerate()
            .map(|(source_id, source)| (source_id as u32, source.to_string()))
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Self::fallback_hvigor_source_map_entries(source_map);
        }

        let mut entries = Vec::with_capacity(sources.len());
        for (source_id, source) in sources {
            let descriptor = HvigorSourceDescriptor::new(
                &source,
                &opts.project_root,
                &opts.module_root,
                source_map.emits_javascript,
                &opts.package_versions,
            )?;
            entries.push(HvigorSourceMapEntry {
                chunk_name: source_map.chunk_name.to_string(),
                mappings: Self::mappings_for_source(
                    &decoded,
                    source_id,
                    &source,
                    source_map.raw_map,
                ),
                source_file: Some(descriptor.source_file),
                source_map_source_file: descriptor.source_map_source_file,
                package_name: descriptor.package_name,
                package_version: descriptor.package_version,
                key_path: descriptor.key_path,
                file: descriptor.file,
                include_source_root: descriptor.include_source_root,
            });
        }
        Ok(entries)
    }

    fn fallback_hvigor_source_map_entries(
        source_map: &ChunkSourceMap<'_>,
    ) -> Result<Vec<HvigorSourceMapEntry>, ArkdownError> {
        let value = Self::validate_real_source_map(source_map.raw_map)?;
        let mappings = value
            .get("mappings")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let source_map_source_file = value
            .get("sources")
            .and_then(|sources| sources.as_array())
            .and_then(|sources| sources.first())
            .and_then(|source| source.as_str())
            .map(str::to_string);
        Ok(vec![HvigorSourceMapEntry {
            chunk_name: source_map.chunk_name.to_string(),
            mappings: mappings.to_string(),
            source_file: None,
            source_map_source_file,
            package_name: None,
            package_version: None,
            key_path: None,
            file: None,
            include_source_root: true,
        }])
    }

    fn mappings_for_source(
        decoded: &RolldownOwnedSourceMap,
        source_id: u32,
        source: &str,
        raw_map: &str,
    ) -> String {
        let mut builder = SourceMapBuilder::default();
        let source_content = decoded
            .get_source_content(source_id)
            .map(|content| content.as_ref())
            .unwrap_or_default();
        builder.set_source_and_content(source, source_content);

        let mut has_source_tokens = false;
        for token in decoded
            .get_tokens()
            .filter(|token| token.get_source_id() == Some(source_id))
        {
            has_source_tokens = true;
            let name_id = token
                .get_name_id()
                .and_then(|id| decoded.get_name(id))
                .map(|name| builder.add_name(name.as_ref()));
            builder.add_token(
                token.get_dst_line(),
                token.get_dst_col(),
                token.get_src_line(),
                token.get_src_col(),
                Some(0),
                name_id,
            );
        }

        if !has_source_tokens {
            return serde_json::from_str::<serde_json::Value>(raw_map)
                .ok()
                .and_then(|value| value.get("mappings")?.as_str().map(str::to_string))
                .unwrap_or_default();
        }

        let value: serde_json::Value =
            serde_json::from_str(&builder.into_sourcemap().to_json_string())
                .unwrap_or_else(|_| serde_json::json!({ "mappings": "" }));
        value
            .get("mappings")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string()
    }
}

struct HvigorSourceDescriptor {
    source_file: String,
    source_map_source_file: Option<String>,
    package_name: Option<String>,
    package_version: Option<String>,
    key_path: Option<String>,
    file: Option<String>,
    include_source_root: bool,
}

impl HvigorSourceDescriptor {
    fn new(
        source: &str,
        project_root: &Path,
        module_root: &Path,
        emits_javascript: bool,
        package_versions: &FxHashMap<String, String>,
    ) -> Result<Self, ArkdownError> {
        let normalized = ArtifactPathSerializer::rel_to_slash(&PathBuf::from(source));
        let components = Path::new(&normalized)
            .components()
            .filter_map(|component| component.as_os_str().to_str().map(str::to_string))
            .collect::<Vec<_>>();

        if let Some(package) = HvigorOhpmSourceDescriptor::new(&components, package_versions)? {
            return Ok(Self {
                source_file: package.source_file,
                source_map_source_file: Some(package.source_map_source_file),
                package_name: Some(package.package_name),
                package_version: Some(package.package_version),
                key_path: None,
                file: None,
                include_source_root: true,
            });
        }

        let source_path = Path::new(&normalized);
        if let Ok(module_relative) = source_path.strip_prefix(module_root) {
            if Self::source_file_after_src_main(&components).is_none() {
                let source_file = ArtifactPathSerializer::rel_to_slash(module_relative);
                let mut key_path = module_relative.to_path_buf();
                if emits_javascript {
                    key_path.set_extension("js");
                }
                let source_map_source_file = source_path
                    .strip_prefix(project_root)
                    .map(ArtifactPathSerializer::rel_to_slash)
                    .unwrap_or_else(|_| {
                        let mut path = PathBuf::from(
                            module_root
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or_default(),
                        );
                        path.push(module_relative);
                        ArtifactPathSerializer::rel_to_slash(&path)
                    });
                return Ok(Self {
                    file: source_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(str::to_string),
                    source_file,
                    source_map_source_file: Some(source_map_source_file),
                    package_name: None,
                    package_version: None,
                    key_path: Some(ArtifactPathSerializer::rel_to_slash(&key_path)),
                    include_source_root: false,
                });
            }
        }

        Ok(Self {
            source_file: Self::source_file_after_src_main(&components)
                .unwrap_or_else(|| normalized.clone()),
            source_map_source_file: None,
            package_name: None,
            package_version: None,
            key_path: None,
            file: None,
            include_source_root: true,
        })
    }

    fn source_file_after_src_main(components: &[String]) -> Option<String> {
        components
            .windows(2)
            .position(|window| window[0] == "src" && window[1] == "main")
            .and_then(|index| Self::join_components(components.get(index + 2..)?))
    }

    fn join_components(components: &[String]) -> Option<String> {
        (!components.is_empty()).then(|| components.join("/"))
    }
}

struct HvigorOhpmSourceDescriptor {
    source_file: String,
    source_map_source_file: String,
    package_name: String,
    package_version: String,
}

impl HvigorOhpmSourceDescriptor {
    fn new(
        components: &[String],
        package_versions: &FxHashMap<String, String>,
    ) -> Result<Option<Self>, ArkdownError> {
        let ohpm_index = components.iter().position(|component| component == ".ohpm");
        let Some(ohpm_index) = ohpm_index else {
            return Ok(None);
        };
        let package_oh_modules_index = components
            .iter()
            .enumerate()
            .skip(ohpm_index + 2)
            .find_map(|(index, component)| (component == "oh_modules").then_some(index));
        let Some(package_oh_modules_index) = package_oh_modules_index else {
            return Ok(None);
        };
        let Some(package_components) = Self::package_components(
            components
                .get(package_oh_modules_index + 1..)
                .unwrap_or_default(),
        ) else {
            return Ok(None);
        };
        let Some(package_name) = HvigorSourceDescriptor::join_components(package_components) else {
            return Ok(None);
        };
        let Some(package_version) = package_versions.get(&package_name).cloned() else {
            return Err(ArkdownError::Bundler(format!(
                "source map package {package_name} is missing package version metadata"
            )));
        };
        let package_end_index = package_oh_modules_index + 1 + package_components.len();
        let source_components = components.get(package_end_index..).unwrap_or_default();
        let source_components = if source_components
            .windows(2)
            .next()
            .is_some_and(|window| window[0] == "src" && window[1] == "main")
        {
            source_components.get(2..).unwrap_or_default()
        } else {
            source_components
        };
        let Some(source_file) = HvigorSourceDescriptor::join_components(source_components) else {
            return Ok(None);
        };

        let source_map_start = components
            .get(..ohpm_index)
            .and_then(|prefix| {
                prefix
                    .iter()
                    .rposition(|component| component == "oh_modules")
            })
            .unwrap_or(ohpm_index);
        let Some(source_map_source_file) = components
            .get(source_map_start..)
            .and_then(HvigorSourceDescriptor::join_components)
        else {
            return Ok(None);
        };

        Ok(Some(Self {
            source_file,
            source_map_source_file,
            package_name,
            package_version,
        }))
    }

    fn package_components(components: &[String]) -> Option<&[String]> {
        let first = components.first()?;
        let len = if first.starts_with('@') { 2 } else { 1 };
        components.get(..len)
    }
}

struct ChunkOutputFileName<'a> {
    chunk_name: &'a str,
}

impl<'a> ChunkOutputFileName<'a> {
    fn new(chunk_name: &'a str) -> Self {
        Self { chunk_name }
    }

    fn js_file_name(&self) -> String {
        ChunkOutputFileNameText::from_parts([self.chunk_name, ".js"])
    }
}

struct ChunkOutputFileNameText;

impl ChunkOutputFileNameText {
    fn from_parts<const N: usize>(parts: [&str; N]) -> String {
        parts.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::ArkChunk;
    use oxc_index::IndexVec;
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
            full_module_is_commonjs: FxHashMap::default(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        }
    }

    #[test]
    fn layout_helpers_point_to_loader_out_ets_artifacts() {
        let loader_out = Path::new("build/default/intermediates/loader_out/default/ets");

        assert_eq!(
            WrittenOutput::abc_path_in(loader_out),
            loader_out.join("modules.abc")
        );
        assert_eq!(
            WrittenOutput::source_maps_path_in(loader_out),
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

        let written =
            WrittenOutput::write_output(&chunk_graph, &output_dir, None).expect("write output");

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

        let written =
            WrittenOutput::write_output(&chunk_graph, &output_dir, None).expect("write output");

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
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/entry"),
            module_name: "entry".to_string(),
            package_name: "entry".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files,
            package_versions: FxHashMap::default(),
        };

        let written = WrittenOutput::write_output(&chunk_graph, &output_dir, Some(&opts))
            .expect("write output");

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
    fn writes_hvigor_source_maps_for_each_original_source() {
        let output_dir = temp_output_dir("hvigor-multi-source-map");
        let source_map = serde_json::json!({
            "version": 3,
            "sources": [
                "src/main/ets/pages/Index.ets",
                "oh_modules/.ohpm/@hw-agconnect+hmcore@1.0.6/oh_modules/@hw-agconnect/hmcore/src/main/ets/base/aegis/AegisAes.ts"
            ],
            "sourcesContent": ["", ""],
            "mappings": ""
        })
        .to_string();
        let chunk_graph = graph(vec![ArkChunk {
            name: "cloud-cV8wJsmc".to_string(),
            code: "console.log('cloud');\n".to_string(),
            source_map: Some(source_map),
            ..ArkChunk::default()
        }]);
        let mut package_versions = FxHashMap::default();
        package_versions.insert("@hw-agconnect/hmcore".to_string(), "1.0.6".to_string());
        let opts = SourceMapOptions {
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/phone"),
            module_name: "phone".to_string(),
            package_name: "phone".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files: FxHashMap::default(),
            package_versions,
        };

        let written = WrittenOutput::write_output(&chunk_graph, &output_dir, Some(&opts))
            .expect("write output");

        let source_maps = written.source_maps_path.expect("sourceMaps.map path");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");
        assert!(
            value
                .get("phone|phone|1.0.0|src/main/ets/pages/Index.ts")
                .is_some(),
            "{value}"
        );
        let agc_key = "phone|@hw-agconnect/hmcore|1.0.6|src/main/ets/base/aegis/AegisAes.ts";
        assert!(value.get(agc_key).is_some(), "{value}");
        assert_eq!(
            value[agc_key]["sources"][0],
            "oh_modules/.ohpm/@hw-agconnect+hmcore@1.0.6/oh_modules/@hw-agconnect/hmcore/src/main/ets/base/aegis/AegisAes.ts"
        );
        assert_eq!(value[agc_key]["entry-package-info"], "phone|1.0.0");
        assert_eq!(value[agc_key]["package-info"], "@hw-agconnect/hmcore|1.0.6");

        let _ = fs_err::remove_dir_all(output_dir);
    }

    #[test]
    fn writes_hvigor_source_maps_for_ohpm_package_without_src_main() {
        let output_dir = temp_output_dir("hvigor-npm-package-source-map");
        let source_map = serde_json::json!({
            "version": 3,
            "sources": [
                "phone/src/main/../oh_modules/.ohpm/@protobufjs+base64@opaquehash=/oh_modules/@protobufjs/base64/index.js"
            ],
            "sourcesContent": [""],
            "mappings": ""
        })
        .to_string();
        let chunk_graph = graph(vec![ArkChunk {
            name: "cloud-cV8wJsmc".to_string(),
            code: "console.log('cloud');\n".to_string(),
            source_map: Some(source_map),
            ..ArkChunk::default()
        }]);
        let mut package_versions = FxHashMap::default();
        package_versions.insert("@protobufjs/base64".to_string(), "1.1.2".to_string());
        let opts = SourceMapOptions {
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/phone"),
            module_name: "phone".to_string(),
            package_name: "phone".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files: FxHashMap::default(),
            package_versions,
        };

        let written = WrittenOutput::write_output(&chunk_graph, &output_dir, Some(&opts))
            .expect("write output");

        let source_maps = written.source_maps_path.expect("sourceMaps.map path");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");
        let key = "phone|@protobufjs/base64|1.1.2|src/main/index.ts";
        assert!(value.get(key).is_some(), "{value}");
        assert_eq!(
            value[key]["sources"][0],
            "oh_modules/.ohpm/@protobufjs+base64@opaquehash=/oh_modules/@protobufjs/base64/index.js"
        );
        assert_eq!(value[key]["package-info"], "@protobufjs/base64|1.1.2");

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

        let err = WrittenOutput::write_output(&chunk_graph, &output_dir, None)
            .expect_err("empty source map rejected");
        assert!(err.to_string().contains("fake empty sourceMaps.map"));

        let _ = fs_err::remove_dir_all(output_dir);
    }
}
