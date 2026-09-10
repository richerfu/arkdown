use rustc_hash::{FxHashMap, FxHashSet};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownError, ChunkGraph, HvigorSourceMapEntry, HvigorSourceMapOptions};
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer};
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

/// One serialized entry in the latest-Hvigor `sourceMaps.map` object.
///
/// The bundler keeps this lightweight provenance alongside the emitted map so
/// the incremental cache can compare source inputs without reading generated
/// source-map files back from disk.
#[derive(Debug, Clone)]
pub struct HvigorSourceMapOutputEntry {
    pub key: String,
    pub source_path: Option<String>,
}

/// Result of serializing an Hvigor-format source-map object.
#[derive(Debug, Clone)]
pub struct HvigorSourceMapOutput {
    pub path: PathBuf,
    pub entries: Vec<HvigorSourceMapOutputEntry>,
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
        Ok(Self::write_hvigor_source_maps_with_metadata_from_raw(
            output_dir,
            source_maps,
            source_map_opts,
        )?
        .map(|output| output.path))
    }

    /// Write an Hvigor-format source-map object and retain the serialized
    /// entry-to-source relation needed by the native incremental cache layer.
    pub fn write_hvigor_source_maps_with_metadata_from_raw<'a, I>(
        output_dir: &Path,
        source_maps: I,
        source_map_opts: &SourceMapOptions,
    ) -> Result<Option<HvigorSourceMapOutput>, ArkdownError>
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
        let entries = Self::write_hvigor_source_maps(&path, &source_maps, source_map_opts)?;
        Ok(Some(HvigorSourceMapOutput { path, entries }))
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
    /// Local source HAR roots that latest module-meta attributes to their own
    /// package identity in source maps, even though they are compiled into the
    /// entry module's ABC.
    pub local_dependency_roots: Vec<SourceMapLocalDependencyRoot>,
    /// Original source IDs that ets-loader emits as JavaScript because a
    /// TypeScript directive selected its JS path. A source map can contain
    /// more than one original source, so the extension cannot be inferred
    /// solely from the map-producing module.
    pub javascript_source_paths: FxHashSet<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct SourceMapLocalDependencyRoot {
    pub root: PathBuf,
    pub package_name: String,
    pub package_version: String,
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
    ) -> Result<Vec<HvigorSourceMapOutputEntry>, ArkdownError> {
        let file = fs_err::File::create(path).map_err(ArkdownError::Io)?;
        let mut writer = BufWriter::with_capacity(1024 * 1024, file);
        let hvigor_opts = HvigorSourceMapOptions::from(opts);
        writer.write_all(b"{").map_err(ArkdownError::Io)?;
        let mut first = true;
        let mut output_entries = Vec::new();
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
                let source_path = Self::source_map_output_source_path(&value);
                serde_json::to_writer(&mut writer, &key).map_err(|e| {
                    ArkdownError::Bundler(format!("failed to serialize sourceMaps.map: {e}"))
                })?;
                writer.write_all(b": ").map_err(ArkdownError::Io)?;
                let mut value_writer = IndentedJsonValueWriter::new(&mut writer, b"  ");
                serde_json::to_writer_pretty(&mut value_writer, &value).map_err(|e| {
                    ArkdownError::Bundler(format!("failed to serialize sourceMaps.map: {e}"))
                })?;
                output_entries.push(HvigorSourceMapOutputEntry { key, source_path });
            }
        }
        if !first {
            writer.write_all(b"\n").map_err(ArkdownError::Io)?;
        }
        writer.write_all(b"}").map_err(ArkdownError::Io)?;
        writer.flush().map_err(ArkdownError::Io)?;
        Ok(output_entries)
    }

    fn source_map_output_source_path(value: &serde_json::Value) -> Option<String> {
        value
            .get("sources")
            .and_then(serde_json::Value::as_array)
            .and_then(|sources| sources.first())
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
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
                &opts.local_dependency_roots,
                &opts.javascript_source_paths,
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
        let source_content = decoded.get_source_content(source_id).unwrap_or_default();
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
        let mappings = value
            .get("mappings")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        Self::preserve_trailing_mapping_lines(mappings, raw_map)
    }

    fn preserve_trailing_mapping_lines(mut mappings: String, raw_map: &str) -> String {
        let raw_line_separators = serde_json::from_str::<serde_json::Value>(raw_map)
            .ok()
            .and_then(|value| value.get("mappings")?.as_str().map(str::to_string))
            .map_or(0, |mappings| {
                mappings.bytes().filter(|byte| *byte == b';').count()
            });
        let mapped_line_separators = mappings.bytes().filter(|byte| *byte == b';').count();
        for _ in mapped_line_separators..raw_line_separators {
            mappings.push(';');
        }
        mappings
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
        local_dependency_roots: &[SourceMapLocalDependencyRoot],
        javascript_source_paths: &FxHashSet<PathBuf>,
    ) -> Result<Self, ArkdownError> {
        let normalized = ArtifactPathSerializer::rel_to_slash(&PathBuf::from(source));
        let emits_javascript = emits_javascript
            || Self::source_emits_javascript(&normalized, project_root, javascript_source_paths);
        if let Some(local_dependency) = Self::local_dependency_descriptor(
            Path::new(&normalized),
            project_root,
            emits_javascript,
            local_dependency_roots,
        ) {
            return Ok(local_dependency);
        }
        let components = Path::new(&normalized)
            .components()
            .filter_map(|component| component.as_os_str().to_str().map(str::to_string))
            .collect::<Vec<_>>();

        if let Some(package) = HvigorOhpmSourceDescriptor::new(&components, package_versions)? {
            let source_file = PathBuf::from(package.source_file);
            let key_path =
                Self::intermediate_source_path(Path::new(&package.key_path), emits_javascript);
            return Ok(Self {
                source_file: ArtifactPathSerializer::rel_to_slash(&source_file),
                source_map_source_file: Some(package.source_map_source_file),
                package_name: Some(package.package_name),
                package_version: Some(package.package_version),
                key_path: Some(ArtifactPathSerializer::rel_to_slash(&key_path)),
                file: None,
                include_source_root: !emits_javascript,
            });
        }

        let source_path = Path::new(&normalized);
        if let Ok(module_relative) = source_path.strip_prefix(module_root) {
            if Self::source_file_after_src_main(&components).is_none() {
                let source_file = ArtifactPathSerializer::rel_to_slash(module_relative);
                let direct_local_dependency = module_relative
                    .components()
                    .next()
                    .is_some_and(|component| component.as_os_str() == "oh_modules");
                let mut key_path = if direct_local_dependency {
                    module_relative
                        .file_name()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| module_relative.to_path_buf())
                } else {
                    module_relative.to_path_buf()
                };
                if emits_javascript {
                    key_path.set_extension("js");
                } else if matches!(
                    source_path
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("ets" | "ts")
                ) {
                    key_path.set_extension("ts");
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
                    // `process_module_files.ts::genContentAndSourceMapInfo`
                    // supplies `sourceRoot: ""` only for the retained TS AST
                    // printer path. `rollup-plugin-ets-typescript.ts` instead
                    // returns TypeScript's emit map when `getShouldEmitJs`
                    // selects JavaScript (for example generated
                    // `ResourceTable.ts` with `//@ts-noCheck`); that map omits
                    // `sourceRoot`. Generated profile compile entries also
                    // omit it on their string-transform path.
                    include_source_root: !emits_javascript
                        && !Self::is_generated_profile_source(&components),
                });
            }
        }

        let mut source_file = PathBuf::from(
            Self::source_file_after_src_main(&components).unwrap_or_else(|| normalized.clone()),
        );
        if emits_javascript {
            source_file.set_extension("js");
        }
        // `SourceMapGenerator` keeps the source path project-relative even
        // when the map key is scoped by a different package identity. A
        // ByteCode HAR is the observable case: the key belongs to its
        // `oh-package.json5#name`, while the original `.ets` still lives below
        // the consuming module directory (for example `entry/src/main/...`).
        // Do not reconstruct this from the package scope; retain the actual
        // project-relative source path that Rollup supplied.
        let source_map_source_file = source_path
            .strip_prefix(project_root)
            .ok()
            .map(ArtifactPathSerializer::rel_to_slash);
        Ok(Self {
            source_file: ArtifactPathSerializer::rel_to_slash(&source_file),
            source_map_source_file,
            package_name: None,
            package_version: None,
            key_path: None,
            file: source_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string),
            include_source_root: !emits_javascript,
        })
    }

    fn source_emits_javascript(
        source: &str,
        project_root: &Path,
        javascript_source_paths: &FxHashSet<PathBuf>,
    ) -> bool {
        let source_path = Path::new(source);
        if matches!(
            source_path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("js" | "mjs" | "cjs")
        ) {
            return true;
        }
        let absolute_source_path = if source_path.is_absolute() {
            source_path.to_path_buf()
        } else {
            project_root.join(source_path)
        };
        javascript_source_paths.contains(&canonicalize_or_owned(&absolute_source_path))
    }

    fn intermediate_source_path(source_path: &Path, emits_javascript: bool) -> PathBuf {
        let mut source_path = source_path.to_path_buf();
        if emits_javascript {
            source_path.set_extension("js");
        } else if matches!(
            source_path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("ets")
        ) {
            source_path.set_extension("ts");
        } else if matches!(
            source_path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("mjs" | "cjs")
        ) {
            source_path.set_extension("js");
        }
        source_path
    }

    /// Matches `generate_sourcemap.js::getPkgInfoByModuleId` for project-model
    /// source HARs. Their module id is outside the consuming module path, so
    /// `module-meta-info-plugin.js::addModuleInfoData` supplies
    /// `dependencyPkgInfo`; the source-map key is relative to the HAR root,
    /// while the map's `sources` entry stays relative to the project root.
    fn local_dependency_descriptor(
        source_path: &Path,
        project_root: &Path,
        emits_javascript: bool,
        local_dependency_roots: &[SourceMapLocalDependencyRoot],
    ) -> Option<Self> {
        let source_path = canonicalize_or_owned(source_path);
        let local_dependency = local_dependency_roots.iter().find(|root| {
            source_path
                .strip_prefix(&root.root)
                .ok()
                .is_some_and(|relative| !relative.as_os_str().is_empty())
        })?;
        let relative = source_path.strip_prefix(&local_dependency.root).ok()?;
        let mut key_path = relative.to_path_buf();
        if emits_javascript {
            key_path.set_extension("js");
        } else if matches!(
            key_path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("ets" | "ts")
        ) {
            key_path.set_extension("ts");
        }
        let source_map_source_file = source_path
            .strip_prefix(project_root)
            .map(ArtifactPathSerializer::rel_to_slash)
            .unwrap_or_else(|_| {
                let mut fallback = local_dependency
                    .root
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_default();
                fallback.push(relative);
                ArtifactPathSerializer::rel_to_slash(&fallback)
            });

        Some(Self {
            source_file: ArtifactPathSerializer::rel_to_slash(relative),
            source_map_source_file: Some(source_map_source_file),
            package_name: Some(local_dependency.package_name.clone()),
            package_version: Some(local_dependency.package_version.clone()),
            key_path: Some(ArtifactPathSerializer::rel_to_slash(&key_path)),
            file: source_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string),
            include_source_root: !emits_javascript,
        })
    }

    fn source_file_after_src_main(components: &[String]) -> Option<String> {
        components
            .windows(2)
            .position(|window| window[0] == "src" && window[1] == "main")
            .and_then(|index| Self::join_components(components.get(index + 2..)?))
    }

    fn is_generated_profile_source(components: &[String]) -> bool {
        components
            .windows(2)
            .any(|window| window[0] == "generated" && window[1] == "profile")
    }

    fn join_components(components: &[String]) -> Option<String> {
        (!components.is_empty()).then(|| components.join("/"))
    }
}

struct HvigorOhpmSourceDescriptor {
    source_file: String,
    key_path: String,
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
        let key_components = components.get(package_end_index..).unwrap_or_default();
        let Some(key_path) = HvigorSourceDescriptor::join_components(key_components) else {
            return Ok(None);
        };
        let source_components = key_components;
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
            key_path,
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

    #[test]
    fn per_source_filter_preserves_rollup_trailing_mapping_lines() {
        let raw_map =
            r#"{"version":3,"sources":["ModuleInfo.ts"],"names":[],"mappings":"AAAA;AACA;"}"#;
        let decoded =
            RolldownOwnedSourceMap::from_json_string(raw_map).expect("source map fixture");

        assert_eq!(
            WrittenOutput::mappings_for_source(&decoded, 0, "ModuleInfo.ts", raw_map),
            "AAAA;AACA;"
        );
    }

    #[test]
    fn original_ohpm_javascript_map_omits_typescript_source_root() {
        let source =
            "/project/oh_modules/.ohpm/@ohos+hypium@1.0.24/oh_modules/@ohos/hypium/index.js";
        let descriptor = HvigorSourceDescriptor::new(
            source,
            Path::new("/project"),
            Path::new("/project/entry"),
            true,
            &FxHashMap::from_iter([("@ohos/hypium".to_string(), "1.0.24".to_string())]),
            &[],
            &FxHashSet::from_iter([PathBuf::from(source)]),
        )
        .expect("describe original package JavaScript");

        assert!(!descriptor.include_source_root);
    }

    #[test]
    fn emitted_typescript_map_omits_typescript_printer_source_root() {
        // `rollup-plugin-ets-typescript.ts::getShouldEmitJs` selects
        // TypeScript emit for `//@ts-noCheck` inputs such as ResourceTable.ts.
        // Its emitted source map has no `sourceRoot`, even though the original
        // source extension is `.ts`.
        let source = "/project/entry/build/default/generated/r/default/ResourceTable.ts";
        let descriptor = HvigorSourceDescriptor::new(
            source,
            Path::new("/project"),
            Path::new("/project/entry"),
            false,
            &FxHashMap::default(),
            &[],
            &FxHashSet::from_iter([PathBuf::from(source)]),
        )
        .expect("describe emitted ResourceTable TypeScript");

        assert!(!descriptor.include_source_root);
        assert_eq!(
            descriptor.key_path.as_deref(),
            Some("build/default/generated/r/default/ResourceTable.js")
        );
    }

    #[test]
    fn project_relative_emitted_typescript_map_uses_project_root_for_language_lookup() {
        // `transform.js::preserveSourceMap` rewrites `sources[0]` relative to
        // `projectRootPath`, while ModuleMode retains the absolute module id.
        let source = "entry/build/default/generated/r/default/ResourceTable.ts";
        let absolute = "/project/entry/build/default/generated/r/default/ResourceTable.ts";
        let descriptor = HvigorSourceDescriptor::new(
            source,
            Path::new("/project"),
            Path::new("/project/entry"),
            false,
            &FxHashMap::default(),
            &[],
            &FxHashSet::from_iter([PathBuf::from(absolute)]),
        )
        .expect("describe project-relative emitted ResourceTable TypeScript");

        assert!(!descriptor.include_source_root);
        assert_eq!(
            descriptor.source_file,
            "entry/build/default/generated/r/default/ResourceTable.js"
        );
    }

    #[test]
    fn emitted_javascript_module_fact_omits_source_root_for_generated_typescript() {
        // `rollup-plugin-ets-typescript.ts::transform` owns the emitted-JS
        // decision before `transform.js::preserveSourceMap` rewrites the map
        // source. The module-scoped fact must therefore be sufficient even
        // when no path-based lookup is available.
        let output_dir = temp_output_dir("emitted-javascript-module-source-map");
        let source_map = serde_json::json!({
            "version": 3,
            "file": "ResourceTable.ts",
            "sources": ["entry/build/default/generated/r/default/ResourceTable.ts"],
            "names": [],
            "mappings": ""
        })
        .to_string();
        let opts = SourceMapOptions {
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/entry"),
            module_name: "entry".to_string(),
            package_name: "entry".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files: FxHashMap::default(),
            package_versions: FxHashMap::default(),
            local_dependency_roots: Vec::new(),
            javascript_source_paths: FxHashSet::default(),
        };

        let output = WrittenOutput::write_hvigor_source_maps_from_raw(
            &output_dir,
            [(
                "/project/entry/build/default/generated/r/default/ResourceTable.ts",
                source_map.as_str(),
                true,
            )],
            &opts,
        )
        .expect("write emitted-JavaScript source map")
        .expect("sourceMaps.map");
        let value: serde_json::Value = serde_json::from_slice(
            &fs_err::read(&output).expect("read emitted-JavaScript source map"),
        )
        .expect("parse emitted-JavaScript source map");
        let key = "entry|entry|1.0.0|entry/build/default/generated/r/default/ResourceTable.js";

        assert!(value[key].get("sourceRoot").is_none(), "{value}");

        let _ = fs_err::remove_dir_all(output_dir);
    }

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
            local_dependency_roots: Vec::new(),
            javascript_source_paths: FxHashSet::default(),
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
            local_dependency_roots: Vec::new(),
            javascript_source_paths: FxHashSet::default(),
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
    fn source_map_extensions_follow_materialized_module_source_language() {
        // `generate_sourcemap.js::getIntermediateModuleId` is per original
        // source, not per Rolldown combined map. A pragma-emitted JS source
        // can therefore share a map with a normal ETS/TS source.
        let output_dir = temp_output_dir("hvigor-source-map-source-language");
        let source_map = serde_json::json!({
            "version": 3,
            "sources": [
                "/project/entry/src/main/ets/pages/Index.ets",
                "/project/entry/src/main/ets/pages/Runtime.ts"
            ],
            "names": [],
            "mappings": "AAAA"
        })
        .to_string();
        let chunk_graph = graph(vec![ArkChunk {
            name: "Index".to_string(),
            code: "export const value = true;\n".to_string(),
            source_map: Some(source_map),
            ..ArkChunk::default()
        }]);
        let opts = SourceMapOptions {
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/entry"),
            module_name: "entry".to_string(),
            package_name: "entry".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files: FxHashMap::default(),
            package_versions: FxHashMap::default(),
            local_dependency_roots: Vec::new(),
            javascript_source_paths: FxHashSet::from_iter([PathBuf::from(
                "/project/entry/src/main/ets/pages/Runtime.ts",
            )]),
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
                .get("entry|entry|1.0.0|src/main/ets/pages/Index.ts")
                .is_some(),
            "{value}"
        );
        assert!(
            value
                .get("entry|entry|1.0.0|src/main/ets/pages/Runtime.js")
                .is_some(),
            "{value}"
        );

        let _ = fs_err::remove_dir_all(output_dir);
    }

    #[test]
    fn bytecode_har_source_map_keeps_consuming_module_source_path() {
        // Latest ets-loader `SourceMapGenerator` scopes the record by the
        // ByteCode HAR package but leaves `sources` relative to the consuming
        // project root. See `generate_sourcemap.js::getIntermediateModuleId`.
        let output_dir = temp_output_dir("bytecode-har-source-map");
        let source_map = serde_json::json!({
            "version": 3,
            "file": "Index.ets",
            "sources": ["/project/entry/src/main/ets/Index.ets"],
            "names": [],
            "mappings": "AAAA"
        })
        .to_string();
        let chunk_graph = graph(vec![ArkChunk {
            name: "Index".to_string(),
            code: "export const bytecodeHar = true;\n".to_string(),
            source_map: Some(source_map),
            ..ArkChunk::default()
        }]);
        let opts = SourceMapOptions {
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/entry"),
            module_name: "bytecode-har".to_string(),
            package_name: "bytecode-har".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files: FxHashMap::default(),
            package_versions: FxHashMap::default(),
            local_dependency_roots: Vec::new(),
            javascript_source_paths: FxHashSet::default(),
        };

        let written = WrittenOutput::write_output(&chunk_graph, &output_dir, Some(&opts))
            .expect("write ByteCode HAR source map");
        let source_maps = written.source_maps_path.expect("sourceMaps.map");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");
        let key = "bytecode-har|bytecode-har|1.0.0|src/main/ets/Index.ts";

        assert_eq!(value[key]["sources"][0], "entry/src/main/ets/Index.ets");
        assert_eq!(value[key]["entry-package-info"], "bytecode-har|1.0.0");

        let _ = fs_err::remove_dir_all(output_dir);
    }

    #[test]
    fn generated_profile_source_map_omits_source_root_like_typescript_string_transform() {
        // `rollup-plugin-ets-typescript.ts::printSourceFile` returns generated
        // profile TS as a string. Unlike `process_module_files.ts`, that path
        // does not create a TypeScript printer map with `sourceRoot: ""`.
        let output_dir = temp_output_dir("generated-profile-source-map");
        let source_map = serde_json::json!({
            "version": 3,
            "file": "ModuleInfo.ts",
            "sources": ["/project/entry/build/default/generated/profile/default/ModuleInfo.ts"],
            "names": [],
            "mappings": "AAAA"
        })
        .to_string();
        let chunk_graph = graph(vec![ArkChunk {
            name: "ModuleInfo".to_string(),
            code: "export const moduleName = 'entry';\n".to_string(),
            source_map: Some(source_map),
            ..ArkChunk::default()
        }]);
        let opts = SourceMapOptions {
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/entry"),
            module_name: "bytecode-har".to_string(),
            package_name: "bytecode-har".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files: FxHashMap::default(),
            package_versions: FxHashMap::default(),
            local_dependency_roots: Vec::new(),
            javascript_source_paths: FxHashSet::from_iter([PathBuf::from(
                "/project/entry/build/default/generated/profile/default/ModuleInfo.ts",
            )]),
        };

        let written = WrittenOutput::write_output(&chunk_graph, &output_dir, Some(&opts))
            .expect("write generated profile source map");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(written.source_maps_path.expect("sourceMaps.map"))
                .expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");
        let key =
            "bytecode-har|bytecode-har|1.0.0|build/default/generated/profile/default/ModuleInfo.js";

        assert!(value[key].get("sourceRoot").is_none(), "{value}");
        assert_eq!(
            value[key]["sources"][0],
            "entry/build/default/generated/profile/default/ModuleInfo.ts"
        );

        let _ = fs_err::remove_dir_all(output_dir);
    }

    #[test]
    fn writes_latest_hvigor_key_for_direct_local_har_source() {
        let output_dir = temp_output_dir("hvigor-local-har-source-map");
        let source_map = serde_json::json!({
            "version": 3,
            "file": "Index.ets",
            "sources": ["/project/entry/oh_modules/share-config-har/Index.ets"],
            "names": [],
            "mappings": "AAAA"
        })
        .to_string();
        let chunk_graph = graph(vec![ArkChunk {
            name: "Index".to_string(),
            code: "export const dependencyValue = 'dependency';\n".to_string(),
            source_map: Some(source_map),
            ..ArkChunk::default()
        }]);
        let opts = SourceMapOptions {
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/entry"),
            module_name: "entry".to_string(),
            package_name: "entry".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files: FxHashMap::default(),
            package_versions: FxHashMap::default(),
            local_dependency_roots: Vec::new(),
            javascript_source_paths: FxHashSet::default(),
        };

        let written = WrittenOutput::write_output(&chunk_graph, &output_dir, Some(&opts))
            .expect("write output");
        let source_maps = written.source_maps_path.expect("sourceMaps.map path");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");
        let key = "entry|entry|1.0.0|Index.ts";

        assert_eq!(
            value[key]["sources"][0],
            "entry/oh_modules/share-config-har/Index.ets"
        );
        assert_eq!(value[key]["sourceRoot"], "");

        let _ = fs_err::remove_dir_all(output_dir);
    }

    #[test]
    fn writes_hvigor_source_map_scope_for_project_model_source_har() {
        // `generate_sourcemap.js::getPkgInfoByModuleId` makes the key package
        // relative and uses dependencyPkgInfo when this project-model HAR is
        // outside the consumer's modulePath.
        let output_dir = temp_output_dir("hvigor-project-model-source-har-source-map");
        let source_map = serde_json::json!({
            "version": 3,
            "sources": [
                "/project/features/discover/Index.ets",
                "/project/features/discover/src/main/ets/views/DiscoverView.ets"
            ],
            "names": [],
            "mappings": "AAAA"
        })
        .to_string();
        let chunk_graph = graph(vec![ArkChunk {
            name: "Discover".to_string(),
            code: "export const discover = true;\n".to_string(),
            source_map: Some(source_map),
            ..ArkChunk::default()
        }]);
        let opts = SourceMapOptions {
            project_root: PathBuf::from("/project"),
            module_root: PathBuf::from("/project/phone"),
            module_name: "phone".to_string(),
            package_name: "phone".to_string(),
            version: "1.0.0".to_string(),
            chunk_source_files: FxHashMap::default(),
            package_versions: FxHashMap::default(),
            local_dependency_roots: vec![SourceMapLocalDependencyRoot {
                root: PathBuf::from("/project/features/discover"),
                package_name: "discover".to_string(),
                package_version: "1.0.0".to_string(),
            }],
            javascript_source_paths: FxHashSet::default(),
        };

        let written = WrittenOutput::write_output(&chunk_graph, &output_dir, Some(&opts))
            .expect("write output");
        let source_maps = written.source_maps_path.expect("sourceMaps.map path");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");

        let root_key = "phone|discover|1.0.0|Index.ts";
        assert_eq!(value[root_key]["sources"][0], "features/discover/Index.ets");
        assert_eq!(value[root_key]["package-info"], "discover|1.0.0");
        assert_eq!(value[root_key]["entry-package-info"], "phone|1.0.0");

        let source_key = "phone|discover|1.0.0|src/main/ets/views/DiscoverView.ts";
        assert_eq!(
            value[source_key]["sources"][0],
            "features/discover/src/main/ets/views/DiscoverView.ets"
        );
        assert_eq!(value[source_key]["package-info"], "discover|1.0.0");

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
            local_dependency_roots: Vec::new(),
            javascript_source_paths: FxHashSet::default(),
        };

        let written = WrittenOutput::write_output(&chunk_graph, &output_dir, Some(&opts))
            .expect("write output");

        let source_maps = written.source_maps_path.expect("sourceMaps.map path");
        let value: serde_json::Value = serde_json::from_str(
            &fs_err::read_to_string(source_maps).expect("read sourceMaps.map"),
        )
        .expect("parse sourceMaps.map");
        let key = "phone|@protobufjs/base64|1.1.2|index.js";
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
