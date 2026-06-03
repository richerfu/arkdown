use std::path::{Path, PathBuf};

use arkdown_core::HvigorSourceMapScope;
use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::cache::{derive_key, CacheFile};
use crate::emit::file::{version_for_target_api, AbcFile};
use crate::error::AbcError;
use crate::ir::builder;
use crate::npm_entries::load_npm_entry_records;
use crate::widget::WidgetCompileConfig;

/// Options controlling how the [`AbcCompiler`] compiles JavaScript to Panda
/// bytecode.
pub struct AbcCompileOptions {
    /// Embed debug information (source positions, variable names) in the output.
    pub debug_info: bool,
    /// Optimisation level: 0 = none, 1 = basic, 2 = full.
    pub opt_level: u8,
    /// Target OpenHarmony API version (e.g. `12` for API 12 / HarmonyOS 4.x).
    pub target_api_version: u32,
    /// Optional target API stage/sub-version passed as `--target-api-sub-version`.
    pub target_api_sub_version: Option<String>,
    /// Merge all records into a single `.abc` output file.
    pub merge_abc: bool,
    /// Parse and lower inputs as ECMAScript modules, matching upstream
    /// `es2abc --module`.
    pub module: bool,
    /// Optional path to an incremental build cache directory.
    pub cache_path: Option<PathBuf>,
    /// Optional hvigor/es2abc `--npm-module-entry-list` file.
    ///
    /// Upstream `CompileNpmModuleEntryList` lowers each line into a standalone
    /// `pandasm::Record` with one `u8=false` field; it is not parsed as JS.
    pub npm_entries: Option<PathBuf>,
}

/// An input source file to compile.
pub struct AbcModule {
    /// Record name — typically the OHM URL of the chunk
    /// (`@bundle:<bundleName>/<moduleName>/<relpath>`).
    pub record_name: String,
    /// JavaScript/TypeScript source passed to the bytecode compiler.
    ///
    /// Upstream `es2abc` accepts normal files directly; when `--module` is set,
    /// the input is parsed as an ECMAScript module. IIFE-wrapped CommonJS is
    /// supported only as a legacy/non-module compatibility shape.
    pub source: String,
    /// Optional source-map JSON string accepted for API compatibility.
    ///
    /// This is not used to derive ABC DebugInfo line/column coordinates.
    pub source_map: Option<String>,
    /// Optional debug source-file name stored in the ABC DebugInfo section.
    ///
    /// Upstream `es2abc` gets this from `filesInfo.txt`; source maps are not
    /// inputs to `BuildLineNumberProgram()`.
    pub source_file: Option<String>,
    /// Package name stored in the module metadata `pkgName@...` field.
    ///
    /// Upstream `es2abc` reads this from column 5 of `filesInfo.txt` and does
    /// not derive it from the record name.
    pub package_name: Option<String>,
    /// Whether this record is compiled from a CommonJS source file.
    ///
    /// Upstream `es2abc` reads this from column 3 of `filesInfo.txt` and writes
    /// it to the module metadata `isCommonjs` field.
    pub is_commonjs: bool,
}

/// Output produced by a successful compilation run.
pub struct AbcArtifactResult {
    /// Path of the written `.abc` file.
    pub path: PathBuf,
    /// API version the file was compiled for.
    pub api_version: u32,
    /// Whether the file contains embedded debug information.
    pub has_debug_info: bool,
}

/// Strip TypeScript type annotations from source before bytecode generation.
///
/// The bundler pipeline may emit TypeScript-flavoured code when the OXC parser
/// falls back to raw source for `.ets` files. This function normalises source
/// through OXC before bytecode lowering.
///
/// Normalise `source` through a TypeScript parse + Codegen round-trip.
///
/// The primary purpose is to remove non-standard ArkTS/ETS constructs that
/// would confuse the downstream `SourceType::ts()` parser used by
/// `build_record`. Standard TypeScript annotations (`): void`, `: Want`)
/// survive the round-trip unchanged — that is intentional: `build_record`
/// now accepts TypeScript source directly via `SourceType::ts()`, so we
/// no longer need to fully strip the annotations here.
///
/// Falls back to the original source if the TS parser panics hard (e.g.
/// truly malformed input).
fn strip_typescript_types(source: &str) -> String {
    let allocator = Allocator::default();
    let parse_ret = Parser::new(&allocator, source, SourceType::ts()).parse();
    if parse_ret.panicked {
        return source.to_string();
    }
    Codegen::new().build(&parse_ret.program).code
}

fn source_file_from_record_name(record_name: &str) -> String {
    HvigorSourceMapScope::source_file_from_record_name(record_name).unwrap_or_else(|| {
        record_name
            .strip_prefix("@bundle:")
            .unwrap_or(record_name)
            .to_string()
    })
}

fn cache_key_inputs(
    modules: &[AbcModule],
    npm_entries_source: Option<&str>,
    options: &AbcCompileOptions,
) -> Vec<(String, String)> {
    let mut inputs: Vec<(String, String)> = modules
        .iter()
        .map(|module| {
            (
                module.record_name.clone(),
                format!(
                    "{}\0source_file={}\0package_name={}\0is_commonjs={}",
                    module.source,
                    module.source_file.as_deref().unwrap_or_default(),
                    module.package_name.as_deref().unwrap_or_default(),
                    module.is_commonjs
                ),
            )
        })
        .collect();
    inputs.push((
        "__arkdown_compile_options__".to_string(),
        format!(
            "target_api_version={}\0target_api_sub_version={}\0module={}\0debug_info={}\0opt_level={}",
            options.target_api_version,
            options
                .target_api_sub_version
                .as_deref()
                .unwrap_or_default(),
            options.module,
            options.debug_info,
            // opt_level affects whether debug info is emitted (>=1 suppresses it)
            // so it must be part of the cache key to avoid mixing debug/release entries.
            options.opt_level,
        ),
    ));
    if let Some(source) = npm_entries_source {
        inputs.push(("__arkdown_npm_entries__".to_string(), source.to_string()));
    }
    inputs
}

/// Pure-Rust ArkCompiler bytecode compiler.
///
/// Replaces the `es2abc` SDK binary described in SPEC-009. Upstream `es2abc`
/// accepts source files directly; `--module` selects ECMAScript module parsing
/// rather than an IIFE entry contract.
///
/// # Example
///
/// ```rust,ignore
/// use arkdown_abc::{AbcCompiler, AbcCompileOptions, AbcModule};
/// use std::path::Path;
///
/// let compiler = AbcCompiler::new(AbcCompileOptions {
///     debug_info: false,
///     opt_level: 0,
///     target_api_version: 12,
///     target_api_sub_version: None,
///     merge_abc: true,
///     module: true,
///     cache_path: None,
///     npm_entries: None,
/// });
///
/// let result = compiler.compile(
///     vec![AbcModule { record_name: "entry/main".into(), source: "export const x = 1;" .into(), source_map: None, source_file: None, package_name: None }],
///     Path::new("output/modules.abc"),
/// )?;
/// println!("wrote {} bytes to {:?}", std::fs::metadata(&result.path)?.len(), result.path);
/// ```
pub struct AbcCompiler {
    options: AbcCompileOptions,
}

impl AbcCompiler {
    /// Create a new compiler with the given options.
    pub fn new(options: AbcCompileOptions) -> Self {
        AbcCompiler { options }
    }

    /// Run the widget compilation pass.
    ///
    /// This is a second, independent ABC compilation pass for ArkTS widgets
    /// (form extension abilities).  It mirrors `initWidgetDefaultArkCompileConfig`
    /// + `submitArkCompileWork` in hvigor's `ArkCompile.doTaskAction()`:
    ///
    /// - Uses a **separate cache directory** (`config.cache_dir`) so widget
    ///   incremental state does not corrupt the main module cache.
    /// - Writes the output to `config.output` (typically `widgets.abc`), not
    ///   the main `modules.abc` path.
    /// - Warnings are suppressed (`ignoreWarning: true` in hvigor).
    ///
    /// # Errors
    ///
    /// Returns [`AbcError::Io`] if the output directory cannot be created or
    /// the file cannot be written.  Returns [`AbcError::Parse`] if any widget
    /// source contains hard syntax errors.
    pub fn compile_widget(
        &self,
        config: &WidgetCompileConfig,
    ) -> Result<AbcArtifactResult, AbcError> {
        // Ensure the widget cache directory exists.
        if !config.cache_dir.as_os_str().is_empty() {
            fs_err::create_dir_all(&config.cache_dir).map_err(AbcError::Io)?;
        }

        // Build a fresh AbcFile for the widget pass — completely independent
        // from the main module's AbcFile.
        let mut abc_file = AbcFile::new();
        let mut has_debug_info = false;

        for input in &config.inputs {
            if !input.path.exists() {
                // Skip missing widget source files; hvigor sets ignoreWarning=true
                // so we mirror that leniency here.
                continue;
            }
            let source = fs_err::read_to_string(&input.path).map_err(AbcError::Io)?;
            let js_source = strip_typescript_types(&source);
            let source_file = input
                .source_file
                .as_deref()
                .map(str::to_string)
                .or_else(|| {
                    config
                        .debug_info
                        .then(|| source_file_from_record_name(&input.record_name))
                });
            has_debug_info |= source_file.is_some();
            let record = builder::build_record_with_debug_and_package_for_target(
                &js_source,
                &input.record_name,
                &mut abc_file.strings,
                &mut abc_file.literal_arrays,
                source_file.as_deref(),
                None,
                false,
                // Widget source files are always compiled as ESM modules.
                true,
                config.target_api_version,
                None,
            )?;
            abc_file.add_record(record);
        }

        let bytes = abc_file
            .serialize_with_version(version_for_target_api(config.target_api_version, None));

        if let Some(parent) = config.output.parent() {
            if !parent.as_os_str().is_empty() {
                fs_err::create_dir_all(parent).map_err(AbcError::Io)?;
            }
        }
        std::fs::write(&config.output, &bytes).map_err(AbcError::Io)?;

        Ok(AbcArtifactResult {
            path: config.output.clone(),
            api_version: config.target_api_version,
            has_debug_info,
        })
    }

    /// Compile `modules` and write the result to `output`.
    ///
    /// Each module in `modules` becomes one [`ClassRecord`][crate::emit::record::ClassRecord]
    /// in the output `.abc` file.  If `merge_abc` is `true` (the common case),
    /// all records are combined into a single file at `output`; otherwise a
    /// per-record layout could be used (TODO for SPEC-009 v2).
    ///
    /// When `options.cache_path` is set the compiler checks an on-disk cache
    /// keyed by `SHA-256(all_sources + options)` before running the IR lowering
    /// pass.  A cache hit skips `build_record` entirely and writes the cached
    /// bytecode to `output`.  A cache miss compiles normally and stores the
    /// result for future runs.
    ///
    /// Returns [`AbcError::Io`] if the output directory cannot be created or
    /// the file cannot be written.  Returns [`AbcError::Parse`] if any module
    /// source contains hard syntax errors.
    pub fn compile(
        &self,
        modules: Vec<AbcModule>,
        output: &Path,
    ) -> Result<AbcArtifactResult, AbcError> {
        let npm_entries_source = self
            .options
            .npm_entries
            .as_ref()
            .map(fs_err::read_to_string)
            .transpose()
            .map_err(AbcError::Io)?;

        // Upstream es2abc (options.cpp:826): debug info is suppressed at opt_level >= 1.
        //   compilerOptions_.debugInfo = IsDebugMode() && GetOptLevel() == 0
        // Computing this once here ensures both the cache-hit return value and the
        // full-compile path agree on whether debug info was emitted.
        let effective_debug_info = self.options.debug_info && self.options.opt_level == 0;

        tracing::debug!(
            module_count = modules.len(),
            npm_entries = ?self.options.npm_entries,
            debug_info = self.options.debug_info,
            effective_debug_info,
            opt_level = self.options.opt_level,
            "abc compile start"
        );

        // ── Incremental cache check (SPEC-025) ────────────────────────────────
        //
        // Derive a cache key from all module sources and the relevant compile
        // options.  If we find a matching entry in the cache file, write the
        // cached bytecode directly to `output` and return early — skipping all
        // IR lowering.
        if let Some(ref cache_dir) = self.options.cache_path {
            let key_inputs_owned =
                cache_key_inputs(&modules, npm_entries_source.as_deref(), &self.options);
            let key_inputs: Vec<(&str, &str)> = key_inputs_owned
                .iter()
                .map(|(record_name, source)| (record_name.as_str(), source.as_str()))
                .collect();
            let key = derive_key(
                &key_inputs,
                self.options.module,
                self.options.debug_info,
                self.options.target_api_version,
            );

            let cache = CacheFile::load(cache_dir);
            if let Some(cached_bytes) = cache.lookup(&key) {
                tracing::debug!(cache = %cache_dir.display(), "abc cache hit");
                // Cache hit — write cached bytecode and return.
                if let Some(parent) = output.parent() {
                    if !parent.as_os_str().is_empty() {
                        fs_err::create_dir_all(parent).map_err(AbcError::Io)?;
                    }
                }
                std::fs::write(output, &cached_bytes).map_err(AbcError::Io)?;
                return Ok(AbcArtifactResult {
                    path: output.to_path_buf(),
                    api_version: self.options.target_api_version,
                    has_debug_info: effective_debug_info,
                });
            }
            tracing::debug!(cache = %cache_dir.display(), "abc cache miss");
        }

        // ── Full compilation ──────────────────────────────────────────────────
        if self.options.opt_level >= 2 {
            // TODO(SPEC-009 v3): implement register allocation for opt_level=2.
            // opt_level=2 should strip debug info (already done via effective_debug_info)
            // AND apply register allocation improvements, but the latter is not yet
            // implemented — warn so release builds are visibly aware of the gap.
            tracing::warn!(
                opt_level = self.options.opt_level,
                "opt_level=2: debug info suppressed, but register allocation is not yet applied"
            );
        }

        let mut abc_file = AbcFile::new();
        let mut has_debug_info = false;

        for module in &modules {
            // Strip TypeScript type annotations before parsing as plain JS.
            // The codegen fallback in the loader may preserve TS syntax when
            // the OXC parser cannot handle `.ets`-specific constructs.
            let js_source = if module.source_file.is_some() {
                module.source.clone()
            } else {
                strip_typescript_types(&module.source)
            };
            // When effective_debug_info is false (opt_level >= 1 or debug_info=false),
            // suppress all debug-info emission by setting source_file to None.
            // Upstream: compilerOptions_.debugInfo=false disables DebugInfoSection entirely.
            let source_file = effective_debug_info.then(|| {
                module
                    .source_file
                    .as_deref()
                    .map(|sf| Self::normalize_source_file(&module.record_name, sf.to_string()))
                    .unwrap_or_else(|| source_file_from_record_name(&module.record_name))
            });
            has_debug_info |= source_file.is_some();
            let record = builder::build_record_with_debug_and_package_for_target(
                &js_source,
                &module.record_name,
                &mut abc_file.strings,
                &mut abc_file.literal_arrays,
                source_file.as_deref(),
                module.package_name.as_deref(),
                module.is_commonjs,
                self.options.module,
                self.options.target_api_version,
                self.options.target_api_sub_version.as_deref(),
            )?;
            abc_file.add_record(record);
        }

        if let Some(path) = &self.options.npm_entries {
            let records = load_npm_entry_records(path)?;
            tracing::debug!(
                npm_entries = %path.display(),
                record_count = records.len(),
                "lowered npm module entry records"
            );
            for record in records {
                abc_file.add_record(record);
            }
        }

        let bytes = abc_file.serialize_with_version(version_for_target_api(
            self.options.target_api_version,
            self.options.target_api_sub_version.as_deref(),
        ));

        // Ensure the output directory exists.
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs_err::create_dir_all(parent).map_err(AbcError::Io)?;
            }
        }
        std::fs::write(output, &bytes).map_err(AbcError::Io)?;

        // ── Write result to cache (SPEC-025) ──────────────────────────────────
        if let Some(ref cache_dir) = self.options.cache_path {
            let key_inputs_owned =
                cache_key_inputs(&modules, npm_entries_source.as_deref(), &self.options);
            let key_inputs: Vec<(&str, &str)> = key_inputs_owned
                .iter()
                .map(|(record_name, source)| (record_name.as_str(), source.as_str()))
                .collect();
            let key = derive_key(
                &key_inputs,
                self.options.module,
                self.options.debug_info,
                self.options.target_api_version,
            );
            let mut cache = CacheFile::load(cache_dir);
            cache.upsert(key, &bytes);
            // Ignore cache write errors — they are non-fatal.
            let _ = cache.save(cache_dir);
        }

        Ok(AbcArtifactResult {
            path: output.to_path_buf(),
            api_version: self.options.target_api_version,
            has_debug_info,
        })
    }

    fn normalize_source_file(record_name: &str, source_file: String) -> String {
        HvigorSourceMapScope::normalize_debug_source_file(record_name, &source_file)
    }
}
