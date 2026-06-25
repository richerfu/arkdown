//! Bytecode compilation stage.
//!
//! Thin wrapper that takes a typed [`BytecodeStageInput`], calls
//! `RolldownBytecodeCompiler`, and returns a typed [`BytecodeStageOutput`].

use std::path::PathBuf;

use anyhow::Result;

use arkdown_core::{HvigorModuleSourceFile, HvigorNpmEntry};

use super::bytecode::{BytecodeAbcInput, RolldownBytecodeCompileInput, RolldownBytecodeCompiler};

/// Input for the bytecode compilation stage for one module.
pub(super) struct BytecodeStageInput {
  /// Pre-built bytecode compiler (backend choice, es2abc path, API version).
  pub(super) compiler: RolldownBytecodeCompiler,
  /// Transformed module sources compiled in hvigor ESMODULE mode.
  pub(super) module_source_files: Vec<HvigorModuleSourceFile>,
  /// External module requests collected from Rolldown module import records.
  pub(super) external_module_requests: Vec<String>,
  /// Package and local dependency mappings written to `npmEntries.txt`.
  pub(super) npm_entries: Vec<HvigorNpmEntry>,
  /// Target path for the output `modules.abc` file.
  pub(super) modules_abc_path: PathBuf,
  /// Hvigor CompileArkTS cache directory used for filesInfo/npmEntries/modules.cache.
  pub(super) cache_dir: PathBuf,
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
        module_source_files: input.module_source_files,
        external_module_requests: input.external_module_requests,
        npm_entries: input.npm_entries,
        modules_abc_path: input.modules_abc_path,
        cache_dir: input.cache_dir,
        cache_file: input.cache_file,
        abc_input_files: input.abc_input_files,
        compile_context_info_path: input.compile_context_info_path,
      })
      .map(|_| ())
      .map_err(|e| anyhow::anyhow!("bytecode stage failed: {}", e))
  }
}
