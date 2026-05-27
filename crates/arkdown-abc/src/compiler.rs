use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::emit::file::AbcFile;
use crate::error::AbcError;
use crate::ir::builder;

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
    /// Optional path to an incremental build cache directory.
    pub cache_path: Option<PathBuf>,
}

/// An input JavaScript module to compile.
pub struct AbcModule {
    /// Record name — typically the OHM URL of the chunk
    /// (`@bundle:<bundleName>/<moduleName>/<relpath>`).
    pub record_name: String,
    /// IIFE-wrapped CJS JavaScript source produced by the bundler pipeline.
    pub source: String,
    /// Optional source-map JSON string for the input source.
    pub source_map: Option<String>,
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
/// falls back to raw source for `.ets` files.  `build_record` parses with
/// `SourceType::mjs()` which rejects TS syntax (`: Type`, `): void`, etc.).
/// This function re-parses as TypeScript and re-emits plain JavaScript via
/// `oxc_codegen`, removing all type annotations in one pass.
///
/// If TypeScript parsing also fails (malformed source), we fall back to
/// plain-JS parsing; if that too fails we return the input unchanged and let
/// `build_record` surface the error with a proper diagnostic.
/// Normalise `source` through a TSX parse + Codegen round-trip.
///
/// The primary purpose is to remove non-standard ArkTS/ETS constructs that
/// would confuse the downstream `SourceType::tsx()` parser used by
/// `build_record`.  Standard TypeScript annotations (`): void`, `: Want`)
/// survive the round-trip unchanged — that is intentional: `build_record`
/// now accepts TypeScript source directly via `SourceType::tsx()`, so we
/// no longer need to fully strip the annotations here.
///
/// Falls back to the original source if the TSX parser panics hard (e.g.
/// truly malformed input).
fn strip_typescript_types(source: &str) -> String {
    let allocator = Allocator::default();
    let parse_ret =
        Parser::new(&allocator, source, SourceType::tsx()).parse();
    if parse_ret.panicked {
        return source.to_string();
    }
    Codegen::new().build(&parse_ret.program).code
}

/// Pure-Rust ArkCompiler bytecode compiler.
///
/// Replaces the `es2abc` SDK binary described in SPEC-009.  The compiler
/// accepts already-bundled, IIFE-wrapped CJS JavaScript (produced by
/// `arkdown` + `arkdown-plugin-chunk-wrapper`) and emits a Panda `.abc` file.
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
///     cache_path: None,
/// });
///
/// let result = compiler.compile(
///     vec![AbcModule { record_name: "entry/main".into(), source: "(function(){})()" .into(), source_map: None }],
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

    /// Compile `modules` and write the result to `output`.
    ///
    /// Each module in `modules` becomes one [`ClassRecord`][crate::emit::record::ClassRecord]
    /// in the output `.abc` file.  If `merge_abc` is `true` (the common case),
    /// all records are combined into a single file at `output`; otherwise a
    /// per-record layout could be used (TODO for SPEC-009 v2).
    ///
    /// Returns [`AbcError::Io`] if the output directory cannot be created or
    /// the file cannot be written.  Returns [`AbcError::Parse`] if any module
    /// source contains hard syntax errors.
    pub fn compile(
        &self,
        modules: Vec<AbcModule>,
        output: &Path,
    ) -> Result<AbcArtifactResult, AbcError> {
        let mut abc_file = AbcFile::new();

        for module in &modules {
            // Strip TypeScript type annotations before parsing as plain JS.
            // The codegen fallback in the loader may preserve TS syntax when
            // the OXC parser cannot handle `.ets`-specific constructs.
            let js_source = strip_typescript_types(&module.source);
            let record = builder::build_record(
                &js_source,
                &module.record_name,
                &mut abc_file.strings,
            )?;
            abc_file.add_record(record);
        }

        let bytes = abc_file.serialize();

        // Ensure the output directory exists.
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs_err::create_dir_all(parent)
                    .map_err(|e| AbcError::Io(e.into()))?;
            }
        }
        std::fs::write(output, &bytes).map_err(AbcError::Io)?;

        Ok(AbcArtifactResult {
            path: output.to_path_buf(),
            api_version: self.options.target_api_version,
            has_debug_info: self.options.debug_info,
        })
    }
}
