use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::cache::{derive_key, CacheFile};
use crate::emit::file::AbcFile;
use crate::error::AbcError;
use crate::ir::builder;
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
    /// Merge all records into a single `.abc` output file.
    pub merge_abc: bool,
    /// Parse and lower inputs as ECMAScript modules, matching upstream
    /// `es2abc --module`.
    pub module: bool,
    /// Optional path to an incremental build cache directory.
    pub cache_path: Option<PathBuf>,
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

/// Build the source file string in es2panda format:
/// `"<moduleName>|<moduleName>|1.0.0|src/main/ets/<relpath>.ts"`
///
/// From golden binary analysis: "entry|entry|1.0.0|src/main/ets/pages/Index.ts"
/// The format matches es2panda's SourceFile() output for OHOS modules.
fn source_file_from_record_name(record_name: &str) -> String {
    // record_name = "com.example.app/entry/ets/pages/Index"
    // or "@bundle:com.example.app/entry/ets/pages/Index"
    let rn = record_name.strip_prefix("@bundle:").unwrap_or(record_name);
    // Split: "com.example.app/entry/ets/pages/Index"
    // parts[0] = bundleName, parts[1] = moduleName, rest = ets path
    let parts: Vec<&str> = rn.splitn(3, '/').collect();
    if parts.len() >= 3 {
        let module_name = parts[1];
        let ets_rel = parts[2]; // "ets/pages/Index"
                                // Convert "ets/pages/Index" → "src/main/ets/pages/Index.ts"
        let src_path = if ets_rel.starts_with("ets/") {
            format!("src/main/{}.ts", ets_rel)
        } else {
            format!("src/main/ets/{}.ts", ets_rel)
        };
        format!("{module_name}|{module_name}|1.0.0|{src_path}")
    } else {
        rn.to_string()
    }
}

fn normalize_source_file(record_name: &str, source_file: String) -> String {
    if source_file.contains('|') {
        return source_file;
    }
    let source_file = source_file.trim_start_matches("./");
    if !(source_file.starts_with("ets/") || source_file.starts_with("src/main/ets/")) {
        return source_file.to_string();
    }

    let rn = record_name.strip_prefix("@bundle:").unwrap_or(record_name);
    let parts: Vec<&str> = rn.splitn(3, '/').collect();
    let module_name = parts.get(1).copied().unwrap_or("entry");
    let src_path = if source_file.starts_with("src/main/") {
        with_ts_extension(source_file)
    } else {
        format!("src/main/{}", with_ts_extension(source_file))
    };
    format!("{module_name}|{module_name}|1.0.0|{src_path}")
}

fn with_ts_extension(path: &str) -> String {
    path.trim_end_matches(".ets")
        .trim_end_matches(".js")
        .trim_end_matches(".ts")
        .to_string()
        + ".ts"
}

fn cache_key_inputs(modules: &[AbcModule]) -> Vec<(String, String)> {
    modules
        .iter()
        .map(|module| {
            (
                module.record_name.clone(),
                format!(
                    "{}\0source_file={}",
                    module.source,
                    module.source_file.as_deref().unwrap_or_default()
                ),
            )
        })
        .collect()
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
///     merge_abc: true,
///     module: true,
///     cache_path: None,
/// });
///
/// let result = compiler.compile(
///     vec![AbcModule { record_name: "entry/main".into(), source: "export const x = 1;" .into(), source_map: None, source_file: None }],
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
            let record = builder::build_record_with_debug(
                &js_source,
                &input.record_name,
                &mut abc_file.strings,
                &mut abc_file.literal_arrays,
                source_file.as_deref(),
                // Widget source files are always compiled as ESM modules.
                true,
            )?;
            abc_file.add_record(record);
        }

        let bytes = abc_file.serialize();

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
        // ── Incremental cache check (SPEC-025) ────────────────────────────────
        //
        // Derive a cache key from all module sources and the relevant compile
        // options.  If we find a matching entry in the cache file, write the
        // cached bytecode directly to `output` and return early — skipping all
        // IR lowering.
        if let Some(ref cache_dir) = self.options.cache_path {
            let key_inputs_owned = cache_key_inputs(&modules);
            let key_inputs: Vec<(&str, &str)> = key_inputs_owned
                .iter()
                .map(|(record_name, source)| (record_name.as_str(), source.as_str()))
                .collect();
            let key = derive_key(&key_inputs, self.options.module, self.options.debug_info);

            let cache = CacheFile::load(cache_dir);
            if let Some(cached_bytes) = cache.lookup(&key) {
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
                    has_debug_info: self.options.debug_info,
                });
            }
        }

        // ── Full compilation ──────────────────────────────────────────────────
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
            let source_file = module
                .source_file
                .as_deref()
                .map(|source_file| {
                    normalize_source_file(&module.record_name, source_file.to_string())
                })
                .or_else(|| {
                    self.options
                        .debug_info
                        .then(|| source_file_from_record_name(&module.record_name))
                });
            has_debug_info |= source_file.is_some();
            let record = builder::build_record_with_debug(
                &js_source,
                &module.record_name,
                &mut abc_file.strings,
                &mut abc_file.literal_arrays,
                source_file.as_deref(),
                self.options.module,
            )?;
            abc_file.add_record(record);
        }

        let bytes = abc_file.serialize();

        // Ensure the output directory exists.
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs_err::create_dir_all(parent).map_err(AbcError::Io)?;
            }
        }
        std::fs::write(output, &bytes).map_err(AbcError::Io)?;

        // ── Write result to cache (SPEC-025) ──────────────────────────────────
        if let Some(ref cache_dir) = self.options.cache_path {
            let key_inputs_owned = cache_key_inputs(&modules);
            let key_inputs: Vec<(&str, &str)> = key_inputs_owned
                .iter()
                .map(|(record_name, source)| (record_name.as_str(), source.as_str()))
                .collect();
            let key = derive_key(&key_inputs, self.options.module, self.options.debug_info);
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
}
