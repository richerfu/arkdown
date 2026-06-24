//! Bytecode compilation stage.
//!
//! Thin wrapper that takes a typed [`BytecodeStageInput`], calls
//! `RolldownBytecodeCompiler`, and returns a typed [`BytecodeStageOutput`].

use std::path::PathBuf;

use anyhow::Result;

use arkdown_core::{ChunkSourceFile, HvigorModuleSourceFile, HvigorNpmEntry};
use arkdown_ts2abc::{BytecodeAbcInput, RolldownBytecodeCompileInput, RolldownBytecodeCompiler};

/// Input for the bytecode compilation stage for one module.
pub(super) struct BytecodeStageInput {
  /// Pre-built bytecode compiler (backend choice, es2abc path, API version).
  pub(super) compiler: RolldownBytecodeCompiler,
  /// Legacy chunk files. ESMODULE builds compile `module_source_files`.
  pub(super) chunk_files: Vec<(String, PathBuf)>,
  /// Source path relative to `src/main` for entry chunks.
  pub(super) chunk_source_files: Vec<ChunkSourceFile>,
  /// Transformed module sources compiled in hvigor ESMODULE mode.
  pub(super) module_source_files: Vec<HvigorModuleSourceFile>,
  /// External module requests collected from Rolldown module import records.
  pub(super) external_module_requests: Vec<String>,
  /// Package and local dependency mappings written to `npmEntries.txt`.
  pub(super) npm_entries: Vec<HvigorNpmEntry>,
  /// Target path for the output `modules.abc` file.
  pub(super) modules_abc_path: PathBuf,
  /// Loader output directory that owns `modules.abc` and `sourceMaps.map`.
  pub(super) output_dir: PathBuf,
  /// Hvigor CompileArkTS cache directory used for filesInfo/npmEntries/modules.cache.
  pub(super) cache_dir: PathBuf,
  /// Bundle name from the product configuration.
  pub(super) bundle_name: String,
  /// OHOS module name, e.g. `entry` or `phone`.
  pub(super) module_name: String,
  /// Optional path to the ABC `--cache-file` (protoBin).
  pub(super) cache_file: Option<PathBuf>,
  /// Existing ByteCode HAR `.abc` files passed through upstream ABC input mode.
  pub(super) abc_input_files: Vec<BytecodeAbcInput>,
  /// Optional upstream `compileContextInfo.json` path.
  pub(super) compile_context_info_path: Option<PathBuf>,
}

/// Bytecode compilation stage runner.
pub(super) struct BytecodeStage;

impl BytecodeStage {
  /// Run the bytecode compilation stage for one module.
  pub(super) fn run(input: BytecodeStageInput) -> Result<()> {
    input
      .compiler
      .compile(RolldownBytecodeCompileInput {
        chunk_files: input.chunk_files,
        chunk_source_files: input.chunk_source_files,
        module_source_files: input.module_source_files,
        external_module_requests: input.external_module_requests,
        npm_entries: input.npm_entries,
        modules_abc_path: input.modules_abc_path,
        output_dir: input.output_dir,
        cache_dir: input.cache_dir,
        bundle_name: input.bundle_name,
        module_name: input.module_name,
        cache_file: input.cache_file,
        abc_input_files: input.abc_input_files,
        compile_context_info_path: input.compile_context_info_path,
      })
      .map(|_| ())
      .map_err(|e| anyhow::anyhow!("bytecode stage failed: {}", e))
  }
}
