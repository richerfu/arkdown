use std::path::{Path, PathBuf};

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
            let record = builder::build_record(
                &module.source,
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
