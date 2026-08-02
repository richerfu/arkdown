//! Typed compilation plan for the ABC bytecode stage.
//!
//! [`Ts2abcPlan`] is the single structured input to the ABC compilation stage.
//! It is constructed by the CLI's `BuildPlanBuilder` from session data and
//! transform facts, then passed to [`Ts2abcCompiler`] for execution.
//!
//! The plan owns all inputs needed to drive SDK `es2abc`:
//! - Compile units (source files with their record names and metadata)
//! - Output paths (ABC file, proto binary, filesInfo.txt)
//! - Feature flags (annotations, define-semantic, obfuscation)
//! - Cache manifest inputs
//!
//! No filesystem reads are performed in this module — all paths are
//! pre-resolved by the CLI planner.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `Ts2abcPlan` | `arkts-pack.js::generateConfig` + genAbc args |
//! | `Ts2abcPlanBuilder` | `TaskCreator` target task inputs aggregation |
//! | `CompileUnitPlan` | per-file entry in `filesInfo.txt` (es2abc) |
//! | `RecordNamePlan` | ABC record name carried in hvigor module source output |

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// ModuleSourceInput
// ---------------------------------------------------------------------------

/// Source input variant for a compile unit.
///
/// Each compile unit has one source input kind that determines how the
/// source bytes are delivered to the bytecode compiler.
///
/// # Upstream correspondence
///
/// Corresponds to the per-file entry format in `filesInfo.txt` columns:
/// `<path>;<record>;<module_type>;<source_file>;<pkg_name>;<is_shared>;<lang>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleSourceInput {
  /// Source is read from the filesystem at `path`.
  File { path: PathBuf },
  /// Source is an existing `.abc` file to merge via `--enable-abc-input`.
  AbcFile { path: PathBuf },
}

// ---------------------------------------------------------------------------
// RecordNamePlan
// ---------------------------------------------------------------------------

/// The record name for a compile unit.
///
/// ABC record names identify modules within the merged `.abc` file. The build
/// bytecode stage consumes the explicit record names emitted by the hvigor
/// module-source pipeline.
///
/// # Upstream correspondence
///
/// Corresponds to the `recordName` field in `filesInfo.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordNamePlan {
  /// Explicit record name from hvigor module-source metadata.
  Explicit(String),
}

impl RecordNamePlan {
  /// Resolve to the final record name string used in `filesInfo.txt`.
  pub fn resolve(&self) -> String {
    match self {
      RecordNamePlan::Explicit(name) => name.clone(),
    }
  }
}

// ---------------------------------------------------------------------------
// CompileUnitPlan
// ---------------------------------------------------------------------------

/// A single compile unit for the ABC stage.
///
/// A compile unit corresponds to one source file → one record in the merged
/// `.abc`.  The plan includes all metadata needed to:
///   1. Write one line of `filesInfo.txt`
///   2. Construct the `SourceFile` entry for `AotCompileOptions`
///   3. Track incremental cache validity
///
/// # Upstream correspondence
///
/// Corresponds to one file line in `filesInfo.txt`:
/// `<path>;<record>;<module_type>;<source_file>;<pkg_name>;<is_shared>;<lang>`
#[derive(Debug, Clone)]
pub struct CompileUnitPlan {
  /// The source input (file, inline, or ABC).
  pub source: ModuleSourceInput,
  /// How the ABC record name is derived.
  pub record_name: RecordNamePlan,
  /// The `sourceFile` field in the `filesInfo.txt` (4th column).
  ///
  /// Used for source-map attribution and debug info.  Typically the
  /// hvigor-scoped `"module|pkg|version|src/main/ets/..."` string.
  pub source_file: Option<String>,
  /// Package name (5th column of `filesInfo.txt`).
  pub package_name: Option<String>,
  /// Whether this is a CommonJS module.
  pub is_commonjs: bool,
  /// Whether this is a shared HSP module.
  pub is_shared_module: bool,
  /// Original source language tag (`"ets"`, `"ts"`, `"js"`).
  pub origin_source_lang: String,
  /// Per-input ABC output path for non-merge mode (non-merge filesInfo col 4).
  pub output_path: Option<PathBuf>,
  /// Per-input ProgramCache `.proto` cache file.
  pub cache_file: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Ts2abcOutputPlan
// ---------------------------------------------------------------------------

/// Output paths for the ABC compilation stage.
///
/// # Upstream correspondence
///
/// Corresponds to `--output`, `--outputProto`, `filesInfo.txt` path, and
/// cache file paths passed to `es2abc`.
#[derive(Debug, Clone)]
pub struct Ts2abcOutputPlan {
  /// Path for the merged `.abc` output file.
  pub abc_output: PathBuf,
  /// Path for the proto binary output (ProgramCache).
  pub proto_output: Option<PathBuf>,
  /// Path for the generated `filesInfo.txt` file.
  pub files_info_path: Option<PathBuf>,
  /// Path for the module-level cache file.
  pub cache_file: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Ts2abcPlan
// ---------------------------------------------------------------------------

/// The complete typed plan for the ABC bytecode compilation stage.
///
/// `Ts2abcPlan` is the single input to [`crate::compiler::Ts2abcCompiler`].
/// It is constructed by the CLI's `BuildPlanBuilder` and encapsulates
/// everything `Ts2abcCompiler` needs to produce ABC output without reading
/// project files or session data.
///
/// # Upstream correspondence
///
/// Replaces the ad-hoc `Ts2abcOptions` struct and the CLI
/// `compile_arkts_handoff.rs` temporary data.  Together they correspond to
/// `arkts-pack.js::generateConfig` + `getCompileEnv` + genAbc input
/// contract.
#[derive(Debug, Clone)]
pub struct Ts2abcPlan {
  /// All source files to compile, in dependency order.
  pub compile_units: Vec<CompileUnitPlan>,
  /// Output path configuration.
  pub output: Ts2abcOutputPlan,
  /// API version for the target device (from `compatibleSdkVersion`).
  pub target_api_version: u32,
  /// Optional API sub-version stage (e.g., `"beta1"`).
  pub target_api_sub_version: Option<String>,
  /// Whether to emit debug information in the ABC.
  pub debug_info: bool,
  /// Optimization level (0–3; default 2 for release).
  pub opt_level: u8,
  /// Whether to apply try-catch function optimization.
  pub optimize_try_catch_func: bool,
  /// Whether to produce a merged multi-module `.abc`.
  pub merge_abc: bool,
  /// Whether to compile in ES module mode (`--module`).
  pub module_mode: bool,
  /// Whether to enable ArkTS annotation support (`--enable-annotations`).
  pub enable_annotations: bool,
  /// Whether to use define-semantic for class fields.
  pub use_define_semantic: bool,
  /// Optional path to the `compileContextInfo.json`.
  pub compile_context_info_path: Option<PathBuf>,
  /// Whether to dump dependency relations (for debugging).
  pub dump_deps_info: bool,
  /// Whether to remove redundant file references from output.
  pub remove_redundant_file: bool,
  /// Optional path to the `npmModuleEntryList.txt` for npm entries.
  pub npm_module_entry_list: Option<PathBuf>,
  /// Path recorded for npm entries in `modules.cache`.
  ///
  /// Latest SDK `ModuleMode.generateAbcCacheFilesInfo()` always records this
  /// row, including ByteCode HAR builds where the file itself is deliberately
  /// not generated or passed to es2abc.
  pub npm_module_cache_entry: Option<PathBuf>,
  /// Whether ABC input mode is enabled (merge existing `.abc` files).
  pub enable_abc_input: bool,
}

impl Ts2abcPlan {
  /// Validate that the plan is internally consistent.
  ///
  /// Returns an error if the plan has obvious configuration errors that
  /// would cause the ABC compiler to fail.
  pub fn validate(&self) -> Result<(), crate::cmd::build::bytecode::error::Ts2abcError> {
    if self.compile_units.is_empty() {
      return Err(
        crate::cmd::build::bytecode::error::Ts2abcError::InvalidPlan(
          "plan has no compile units".to_string(),
        ),
      );
    }
    if !self.merge_abc && self.compile_units.len() > 1 {
      // Non-merge mode requires each unit to have its own output path.
      let missing = self
        .compile_units
        .iter()
        .filter(|u| u.output_path.is_none())
        .count();
      if missing > 0 {
        return Err(
          crate::cmd::build::bytecode::error::Ts2abcError::InvalidPlan(format!(
            "{missing} compile unit(s) missing output_path in non-merge mode"
          )),
        );
      }
    }
    Ok(())
  }
}

// ---------------------------------------------------------------------------
// Ts2abcPlanBuilder
// ---------------------------------------------------------------------------

/// Builder for [`Ts2abcPlan`].
///
/// `Ts2abcPlanBuilder` is used by the CLI's `BuildPlanBuilder` to assemble
/// the complete ABC compilation plan from loader metadata, transform facts,
/// and bundle output.
///
/// # Upstream correspondence
///
/// Corresponds to assembling the `generateConfig` input in
/// `arkts-pack.js` and the `genAbc` plugin configuration in
/// `rollup-plugin-gen-abc.js`.
pub struct Ts2abcPlanBuilder {
  compile_units: Vec<CompileUnitPlan>,
  output: Option<Ts2abcOutputPlan>,
  target_api_version: u32,
  target_api_sub_version: Option<String>,
  debug_info: bool,
  opt_level: u8,
  optimize_try_catch_func: bool,
  merge_abc: bool,
  module_mode: bool,
  enable_annotations: bool,
  use_define_semantic: bool,
  compile_context_info_path: Option<PathBuf>,
  dump_deps_info: bool,
  remove_redundant_file: bool,
  npm_module_entry_list: Option<PathBuf>,
  npm_module_cache_entry: Option<PathBuf>,
  enable_abc_input: bool,
}

impl Ts2abcPlanBuilder {
  /// Create a new builder targeting a specific API version.
  pub fn new(target_api_version: u32) -> Self {
    Self {
      compile_units: Vec::new(),
      output: None,
      target_api_version,
      target_api_sub_version: None,
      debug_info: false,
      opt_level: 2,
      optimize_try_catch_func: true,
      merge_abc: true,
      module_mode: true,
      enable_annotations: false,
      use_define_semantic: false,
      compile_context_info_path: None,
      dump_deps_info: false,
      remove_redundant_file: false,
      npm_module_entry_list: None,
      npm_module_cache_entry: None,
      enable_abc_input: false,
    }
  }

  /// Add a pre-resolved compile unit, including its upstream ProgramCache path.
  pub fn with_compile_unit(mut self, compile_unit: CompileUnitPlan) -> Self {
    self.compile_units.push(compile_unit);
    self
  }

  /// Set the output paths.
  pub fn with_output(mut self, output: Ts2abcOutputPlan) -> Self {
    self.output = Some(output);
    self
  }

  /// Enable debug information.
  pub fn with_debug_info(mut self) -> Self {
    self.debug_info = true;
    self
  }

  /// Set the optimization level.
  pub fn with_opt_level(mut self, level: u8) -> Self {
    self.opt_level = level;
    self
  }

  /// Enable ArkTS annotation support.
  pub fn with_annotations(mut self) -> Self {
    self.enable_annotations = true;
    self
  }

  /// Set the npm module entry manifest path passed to SDK es2abc.
  pub fn with_npm_module_entry_list(mut self, path: PathBuf) -> Self {
    self.npm_module_cache_entry = Some(path.clone());
    self.npm_module_entry_list = Some(path);
    self
  }

  /// Record the npm module entry path in `modules.cache` without passing it
  /// to es2abc. Latest hvigor uses this split behavior for ByteCode HAR.
  pub fn with_npm_module_cache_entry(mut self, path: PathBuf) -> Self {
    self.npm_module_cache_entry = Some(path);
    self
  }

  /// Set the upstream compile context file passed as `--compile-context-info`.
  pub fn with_compile_context_info(mut self, path: PathBuf) -> Self {
    self.compile_context_info_path = Some(path);
    self
  }

  /// Enable upstream ABC input merge mode.
  pub fn with_abc_input(mut self) -> Self {
    self.enable_abc_input = true;
    self
  }

  /// Enable upstream redundant-record pruning after ABC input dependency scan.
  pub fn with_remove_redundant_file(mut self) -> Self {
    self.remove_redundant_file = true;
    self
  }

  /// Add an existing `.abc` file to the es2abc input list.
  pub fn with_abc_input_file(
    mut self,
    path: PathBuf,
    package_name: impl Into<String>,
    cache_file: PathBuf,
  ) -> Self {
    self.compile_units.push(CompileUnitPlan {
      source: ModuleSourceInput::AbcFile { path },
      record_name: RecordNamePlan::Explicit(String::new()),
      source_file: None,
      package_name: Some(package_name.into()),
      is_commonjs: false,
      is_shared_module: false,
      origin_source_lang: String::new(),
      output_path: None,
      cache_file: Some(cache_file),
    });
    self
  }

  /// Build the plan.
  ///
  /// Returns an error if required fields are missing or the plan is invalid.
  pub fn build(self) -> Result<Ts2abcPlan, crate::cmd::build::bytecode::error::Ts2abcError> {
    let output = self.output.ok_or_else(|| {
      crate::cmd::build::bytecode::error::Ts2abcError::InvalidPlan(
        "output paths not set".to_string(),
      )
    })?;
    let plan = Ts2abcPlan {
      compile_units: self.compile_units,
      output,
      target_api_version: self.target_api_version,
      target_api_sub_version: self.target_api_sub_version,
      debug_info: self.debug_info,
      opt_level: self.opt_level,
      optimize_try_catch_func: self.optimize_try_catch_func,
      merge_abc: self.merge_abc,
      module_mode: self.module_mode,
      enable_annotations: self.enable_annotations,
      use_define_semantic: self.use_define_semantic,
      compile_context_info_path: self.compile_context_info_path,
      dump_deps_info: self.dump_deps_info,
      remove_redundant_file: self.remove_redundant_file,
      npm_module_entry_list: self.npm_module_entry_list,
      npm_module_cache_entry: self.npm_module_cache_entry,
      enable_abc_input: self.enable_abc_input,
    };
    plan.validate()?;
    Ok(plan)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn plan_validate_fails_on_empty_compile_units() {
    let result = Ts2abcPlanBuilder::new(12)
      .with_output(Ts2abcOutputPlan {
        abc_output: PathBuf::from("/out/modules.abc"),
        proto_output: None,
        files_info_path: None,
        cache_file: None,
      })
      .build();
    // Even with output set, empty compile units should fail validation.
    assert!(result.is_err());
  }
}
