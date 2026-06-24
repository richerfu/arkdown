//! Typed compilation plan for the ABC bytecode stage.
//!
//! [`Ts2abcPlan`] is the single structured input to the ABC compilation stage.
//! It is constructed by the CLI's `BuildPlanBuilder` from session data and
//! transform facts, then passed to [`Ts2abcCompiler`] for execution.
//!
//! The plan owns all inputs needed to drive SDK `es2abc`:
//! - Compile units (source files with their record names and metadata)
//! - Output paths (ABC file, proto binary, filesInfo.txt)
//! - SDK backend selection
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
//! | `BytecodeBackendKind` | SDK backend flag |
//! | `LoaderMetadataInput` | `loader.json` `compileEntry`/`otherCompileFiles` |
//! | `RecordNamePlan` | ABC record name derivation from OHM URL |

use std::path::PathBuf;

use arkdown_core::ids::CompileUnitId;
use arkdown_path::ArtifactPathSerializer;

use crate::error::Ts2abcError;
use crate::file_info::{FileInfoColumnPolicy, FileInfoLine, FileInfoManifestText};

// ---------------------------------------------------------------------------
// BytecodeBackendKind
// ---------------------------------------------------------------------------

/// Which bytecode compilation backend to use.
///
/// # Upstream correspondence
///
/// Corresponds to the runtime backend selection in
/// `load-sdk-functions.js::genAbc` and `arkOptions.runtimeOnly`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BytecodeBackendKind {
    /// Use the SDK's `es2abc` command-line tool via subprocess.
    SdkEs2abc,
}

// ---------------------------------------------------------------------------
// CompileUnitId (re-export)
// ---------------------------------------------------------------------------

// CompileUnitId is defined in arkdown_core::ids; we use it here for plan
// entries.

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
    /// Source is a pre-computed in-memory string (e.g., from transform).
    Inline {
        content: String,
        virtual_path: PathBuf,
    },
    /// Source is an existing `.abc` file to merge via `--enable-abc-input`.
    AbcFile { path: PathBuf },
}

// ---------------------------------------------------------------------------
// RecordNamePlan
// ---------------------------------------------------------------------------

/// The record name policy for a compile unit.
///
/// ABC record names identify modules within the merged `.abc` file.  They are
/// derived either from the OHM URL, the file path relative to the module root,
/// or an explicit override from `loader.json`.
///
/// # Upstream correspondence
///
/// Corresponds to [`RecordNameDeriver`] policy and the `recordName` field in
/// `filesInfo.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordNamePlan {
    /// Use the OHM URL as the record name (preferred with
    /// `useNormalizedOHMUrl: true`).
    OhmUrl(String),
    /// Derive the record name from the source path relative to the module
    /// root (legacy path-based naming).
    FromSourcePath {
        /// Source path relative to the module root.
        rel_path: String,
    },
    /// Explicit record name override (from `loader.json` `recordName` field).
    Explicit(String),
}

impl RecordNamePlan {
    /// Resolve to the final record name string used in `filesInfo.txt`.
    pub fn resolve(&self) -> String {
        match self {
            RecordNamePlan::OhmUrl(url) => url.clone(),
            RecordNamePlan::FromSourcePath { rel_path } => {
                // Remove extension and convert slashes to match hvigor behavior.
                ArtifactPathSerializer::rel_to_slash(
                    &std::path::Path::new(rel_path).with_extension(""),
                )
            }
            RecordNamePlan::Explicit(name) => name.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// LoaderMetadataInput
// ---------------------------------------------------------------------------

/// Loader.json-derived metadata for a single compile unit.
///
/// Captures the fields from `loader.json` `compileEntry`/`otherCompileFiles`
/// that are needed to construct the `filesInfo.txt` line for each module.
///
/// # Upstream correspondence
///
/// Corresponds to a single entry in `loader.json::compileEntry` or
/// `otherCompileFiles` as consumed by `ark_compiler/transform.js`.
#[derive(Debug, Clone)]
pub struct LoaderMetadataInput {
    /// The OHM URL or relative path for this entry.
    pub module_id: String,
    /// The ABC record name (from loader.json `recordName` field if present).
    pub record_name: Option<String>,
    /// Package name from `oh-package.json5` (for `filesInfo.txt` column 5).
    pub package_name: Option<String>,
    /// The ohpm package version (for `bytecodeHar` version tracking).
    pub package_version: Option<String>,
    /// Whether this module is a CommonJS module.
    pub is_commonjs: bool,
    /// Whether this is a shared module (HSP inter-package export).
    pub is_shared_module: bool,
    /// Original source language (`"ets"`, `"ts"`, `"js"`).
    pub origin_source_lang: String,
}

// ---------------------------------------------------------------------------
// BundleJsInput
// ---------------------------------------------------------------------------

/// Bundle-mode JS chunk input for ABC compilation.
///
/// In bundle mode (non-fast-build), the entire JS output of Rolldown is
/// compiled as a single chunk file.
///
/// # Upstream correspondence
///
/// Corresponds to `generate_bundle_abc.js` input: one JS file per chunk.
#[derive(Debug, Clone)]
pub struct BundleJsInput {
    /// Path to the bundled JS chunk file.
    pub js_path: PathBuf,
    /// Chunk name (maps to the ABC record name).
    pub chunk_name: String,
    /// Whether this chunk is the entry chunk.
    pub is_entry: bool,
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
    /// Stable ID for this compile unit within the plan.
    pub id: CompileUnitId,
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
    /// Directory for per-module source maps.
    pub source_map_dir: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Ts2abcParitySnapshot
// ---------------------------------------------------------------------------

/// Debug snapshot for hvigor parity checks.
///
/// Records the complete plan in a serializable form so that CLI parity
/// tests can compare the arkdown plan against what hvigor would have passed
/// to `es2abc`.
///
/// # Upstream correspondence
///
/// Corresponds to the `filesInfo.txt` content and `es2abc` command line that
/// hvigor generates, used for parity validation.
#[derive(Debug, Clone)]
pub struct Ts2abcParitySnapshot {
    /// The `filesInfo.txt` content that would be written.
    pub files_info_content: String,
    /// The `es2abc` command arguments (for SDK backend mode).
    pub sdk_command_args: Vec<String>,
    /// Total number of compile units.
    pub compile_unit_count: usize,
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
    /// Which backend to use.
    pub backend_kind: BytecodeBackendKind,
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
    /// Whether ABC input mode is enabled (merge existing `.abc` files).
    pub enable_abc_input: bool,
}

impl Ts2abcPlan {
    /// Validate that the plan is internally consistent.
    ///
    /// Returns an error if the plan has obvious configuration errors that
    /// would cause the ABC compiler to fail.
    pub fn validate(&self) -> Result<(), crate::error::Ts2abcError> {
        if self.compile_units.is_empty() {
            return Err(crate::error::Ts2abcError::InvalidPlan(
                "plan has no compile units".to_string(),
            ));
        }
        if !self.merge_abc && self.compile_units.len() > 1 {
            // Non-merge mode requires each unit to have its own output path.
            let missing = self
                .compile_units
                .iter()
                .filter(|u| u.output_path.is_none())
                .count();
            if missing > 0 {
                return Err(crate::error::Ts2abcError::InvalidPlan(format!(
                    "{missing} compile unit(s) missing output_path in non-merge mode"
                )));
            }
        }
        Ok(())
    }

    /// Return a reference to all compile units.
    pub fn compile_units(&self) -> &[CompileUnitPlan] {
        &self.compile_units
    }

    /// Return the output plan.
    pub fn output_plan(&self) -> &Ts2abcOutputPlan {
        &self.output
    }

    /// Return the backend kind.
    pub fn backend_kind(&self) -> BytecodeBackendKind {
        self.backend_kind
    }

    /// Produce a parity snapshot for debugging and parity tests.
    ///
    /// # Upstream correspondence
    ///
    /// The snapshot content matches the `filesInfo.txt` format written by
    /// hvigor's `es2abc` wrapper.
    pub fn to_parity_snapshot(&self) -> Result<Ts2abcParitySnapshot, Ts2abcError> {
        let policy = FileInfoColumnPolicy::merge_columns(self.merge_abc);
        let mut manifest = FileInfoManifestText::new();
        for unit in &self.compile_units {
            let line = FileInfoLine::from_compile_unit(unit, policy)?;
            manifest.push_line(&line);
        }

        Ok(Ts2abcParitySnapshot {
            files_info_content: manifest.into_string(),
            sdk_command_args: Vec::new(), // populated by SdkCommandBuilder when applicable
            compile_unit_count: self.compile_units.len(),
        })
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
    backend_kind: BytecodeBackendKind,
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
    enable_abc_input: bool,
    next_unit_id: u32,
}

impl Ts2abcPlanBuilder {
    /// Create a new builder targeting a specific API version.
    pub fn new(target_api_version: u32) -> Self {
        Self {
            compile_units: Vec::new(),
            output: None,
            backend_kind: BytecodeBackendKind::SdkEs2abc,
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
            enable_abc_input: false,
            next_unit_id: 0,
        }
    }

    /// Add a compile unit from loader metadata and a source path.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to processing a single `compileEntry` or
    /// `otherCompileFiles` entry from `loader.json`.
    pub fn from_loader_and_bundle(
        mut self,
        meta: LoaderMetadataInput,
        source_path: PathBuf,
    ) -> Self {
        let unit_id = CompileUnitId::new(self.next_unit_id as usize);
        self.next_unit_id += 1;
        let record_name = meta
            .record_name
            .map(RecordNamePlan::Explicit)
            .unwrap_or_else(|| RecordNamePlan::OhmUrl(meta.module_id.clone()));
        self.compile_units.push(CompileUnitPlan {
            id: unit_id,
            source: ModuleSourceInput::File { path: source_path },
            record_name,
            source_file: None,
            package_name: meta.package_name,
            is_commonjs: meta.is_commonjs,
            is_shared_module: meta.is_shared_module,
            origin_source_lang: meta.origin_source_lang,
            output_path: None,
            cache_file: None,
        });
        self
    }

    /// Add a pre-resolved compile unit.
    ///
    /// This is used by the CLI module pipeline after rolldown has produced
    /// concrete JS chunks and record names. The builder assigns the stable
    /// compile-unit ID so callers do not allocate IDs directly.
    pub fn with_compile_unit(
        mut self,
        source: ModuleSourceInput,
        record_name: RecordNamePlan,
        source_file: Option<String>,
        package_name: Option<String>,
        is_commonjs: bool,
        is_shared_module: bool,
        origin_source_lang: impl Into<String>,
    ) -> Self {
        let unit_id = CompileUnitId::new(self.next_unit_id as usize);
        self.next_unit_id += 1;
        self.compile_units.push(CompileUnitPlan {
            id: unit_id,
            source,
            record_name,
            source_file,
            package_name,
            is_commonjs,
            is_shared_module,
            origin_source_lang: origin_source_lang.into(),
            output_path: None,
            cache_file: None,
        });
        self
    }

    /// Add a pre-resolved compile unit with an upstream ProgramCache path.
    pub fn with_compile_unit_cache(
        mut self,
        source: ModuleSourceInput,
        record_name: RecordNamePlan,
        source_file: Option<String>,
        package_name: Option<String>,
        is_commonjs: bool,
        is_shared_module: bool,
        origin_source_lang: impl Into<String>,
        cache_file: Option<PathBuf>,
    ) -> Self {
        let unit_id = CompileUnitId::new(self.next_unit_id as usize);
        self.next_unit_id += 1;
        self.compile_units.push(CompileUnitPlan {
            id: unit_id,
            source,
            record_name,
            source_file,
            package_name,
            is_commonjs,
            is_shared_module,
            origin_source_lang: origin_source_lang.into(),
            output_path: None,
            cache_file,
        });
        self
    }

    /// Set the output paths.
    pub fn with_output(mut self, output: Ts2abcOutputPlan) -> Self {
        self.output = Some(output);
        self
    }

    /// Use the SDK es2abc backend.
    pub fn with_sdk_backend(mut self) -> Self {
        self.backend_kind = BytecodeBackendKind::SdkEs2abc;
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

    /// Enable use-define semantics for class fields.
    pub fn with_use_define_semantic(mut self) -> Self {
        self.use_define_semantic = true;
        self
    }

    /// Set the npm module entry manifest path passed to SDK es2abc.
    pub fn with_npm_module_entry_list(mut self, path: PathBuf) -> Self {
        self.npm_module_entry_list = Some(path);
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
    pub fn with_abc_input_file(mut self, path: PathBuf, package_name: impl Into<String>) -> Self {
        let unit_id = CompileUnitId::new(self.next_unit_id as usize);
        self.next_unit_id += 1;
        self.compile_units.push(CompileUnitPlan {
            id: unit_id,
            source: ModuleSourceInput::AbcFile { path },
            record_name: RecordNamePlan::Explicit(String::new()),
            source_file: None,
            package_name: Some(package_name.into()),
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
            output_path: None,
            cache_file: None,
        });
        self
    }

    /// Build the plan.
    ///
    /// Returns an error if required fields are missing or the plan is invalid.
    pub fn build(self) -> Result<Ts2abcPlan, crate::error::Ts2abcError> {
        let output = self.output.ok_or_else(|| {
            crate::error::Ts2abcError::InvalidPlan("output paths not set".to_string())
        })?;
        let plan = Ts2abcPlan {
            compile_units: self.compile_units,
            output,
            backend_kind: self.backend_kind,
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
    fn record_name_ohm_url_resolves() {
        let r = RecordNamePlan::OhmUrl("@bundle:com.example/entry/ets/pages/Index".to_string());
        assert_eq!(r.resolve(), "@bundle:com.example/entry/ets/pages/Index");
    }

    #[test]
    fn record_name_from_source_path_strips_extension() {
        let r = RecordNamePlan::FromSourcePath {
            rel_path: "ets/pages/Index.ets".to_string(),
        };
        assert_eq!(r.resolve(), "ets/pages/Index");
    }

    #[test]
    fn plan_validate_fails_on_empty_compile_units() {
        let result = Ts2abcPlanBuilder::new(12)
            .with_output(Ts2abcOutputPlan {
                abc_output: PathBuf::from("/out/modules.abc"),
                proto_output: None,
                files_info_path: None,
                cache_file: None,
                source_map_dir: None,
            })
            .build();
        // Even with output set, empty compile units should fail validation.
        assert!(result.is_err());
    }

    #[test]
    fn parity_snapshot_generates_files_info_content() {
        let plan = Ts2abcPlan {
            compile_units: vec![CompileUnitPlan {
                id: CompileUnitId::new(0),
                source: ModuleSourceInput::File {
                    path: PathBuf::from("/src/Index.js"),
                },
                record_name: RecordNamePlan::Explicit("entry/Index".to_string()),
                source_file: None,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: "ts".to_string(),
                output_path: None,
                cache_file: None,
            }],
            output: Ts2abcOutputPlan {
                abc_output: PathBuf::from("/out/modules.abc"),
                proto_output: None,
                files_info_path: None,
                cache_file: None,
                source_map_dir: None,
            },
            backend_kind: BytecodeBackendKind::SdkEs2abc,
            target_api_version: 12,
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
            enable_abc_input: false,
        };
        let snapshot = plan.to_parity_snapshot().unwrap();
        assert_eq!(snapshot.compile_unit_count, 1);
        assert!(snapshot.files_info_content.contains("entry/Index"));
    }
}
