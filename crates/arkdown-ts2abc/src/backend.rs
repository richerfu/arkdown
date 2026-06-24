//! Bytecode backend trait and implementations.
//!
//! The bytecode backend abstraction decouples the `Ts2abcCompiler` from the
//! native SDK `es2abc` process.
//!
//! The `BytecodeBackendRunner` provides the unified entry point used by
//! `Ts2abcCompiler::compile_unit`.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `BytecodeBackend` trait | SDK `load-sdk-functions.js::genAbc` dispatch |
//! | `SdkEs2abcBackend` | native SDK `es2abc` subprocess |
//! | `BytecodeBackendRunner` | `rollup-plugin-gen-abc.js::generateBundle` |
//! | `SdkCommandBuilder` | `es2abc` command-line argument construction |

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::driver::{Ts2abcInput, Ts2abcOptions, Ts2abcOutput};
use crate::error::Ts2abcError;
use crate::file_info::{FileInfoInput, FileInfoLine, FileInfoManifestPath, FileInfoManifestText};
use crate::plan::Ts2abcPlan;
use crate::records::RecordNameDeriver;

// ---------------------------------------------------------------------------
// BytecodeBackend trait
// ---------------------------------------------------------------------------

/// Abstraction over the bytecode compilation engine.
///
/// A `BytecodeBackend` compiles JS/TS/ETS source files to ABC bytecode through
/// the SDK process boundary.
///
/// # Upstream correspondence
///
/// Corresponds to `load-sdk-functions.js::genAbc` backend dispatch in hvigor.
pub trait BytecodeBackend: Send + Sync {
    /// Compile the given options to ABC bytecode.
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError>;
}

/// The SDK `es2abc` subprocess backend.
///
/// Spawns the native `es2abc` binary from the DevEco Studio SDK.  Used when
/// the Rust backend does not yet support all required flags (e.g.,
/// `--outputProto`, specific `--enable-*` options).
///
/// # Upstream correspondence
///
/// Corresponds to `load-sdk-functions.js::genAbc` when the SDK binary path
/// is provided.
#[derive(Debug, Clone)]
pub struct SdkEs2abcBackend {
    es2abc_path: PathBuf,
    input_file_list: Option<PathBuf>,
}

impl SdkEs2abcBackend {
    /// Create a new backend pointing to the given `es2abc` binary.
    pub fn new(es2abc_path: PathBuf) -> Self {
        Self {
            es2abc_path,
            input_file_list: None,
        }
    }

    /// Set an explicit input file list path (skips per-input filesInfo
    /// generation).
    pub fn with_input_file_list(mut self, input_file_list: Option<PathBuf>) -> Self {
        self.input_file_list = input_file_list;
        self
    }
}

impl BytecodeBackend for SdkEs2abcBackend {
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
        // Delegate to the driver's implementation.
        let backend = crate::driver::SdkEs2abcBackend::new(self.es2abc_path.clone())
            .with_input_file_list(self.input_file_list.clone());
        crate::driver::BytecodeBackend::compile(&backend, options)
    }
}

// ---------------------------------------------------------------------------
// BytecodeBackendRunner
// ---------------------------------------------------------------------------

/// Runner that selects and invokes a `BytecodeBackend`.
///
/// `BytecodeBackendRunner` owns the backend selection logic and is the
/// top-level entry point for the CLI when using the legacy `Ts2abcOptions`
/// path.  New callers should use `Ts2abcCompiler` with a `Ts2abcPlan`.
///
/// # Upstream correspondence
///
/// Corresponds to `rollup-plugin-gen-abc.js::generateBundle` backend
/// dispatch.
pub struct BytecodeBackendRunner {
    backend: Box<dyn BytecodeBackend>,
}

impl BytecodeBackendRunner {
    /// Create a runner using the SDK `es2abc` backend.
    ///
    /// This is the default production backend in the initial parity path.
    pub fn default_sdk(es2abc_path: PathBuf) -> Self {
        Self::sdk_es2abc(es2abc_path)
    }

    /// Create a runner using the SDK es2abc backend.
    pub fn sdk_es2abc(es2abc_path: PathBuf) -> Self {
        Self {
            backend: Box::new(SdkEs2abcBackend::new(es2abc_path)),
        }
    }

    /// Create a runner from a boxed backend.
    pub fn with_backend(backend: Box<dyn BytecodeBackend>) -> Self {
        Self { backend }
    }

    /// Reject compiling without an SDK path.
    pub fn compile_with_default_backend(
        _options: Ts2abcOptions,
    ) -> Result<Ts2abcOutput, Ts2abcError> {
        Err(Ts2abcError::MissingValue(
            "sdk es2abc path is required".to_string(),
        ))
    }

    /// Compile using the runner's selected backend.
    ///
    /// # Upstream correspondence
    ///
    /// Replaces `compile_to_abc_with_backend` free function.
    pub fn compile_with_backend(
        &self,
        options: Ts2abcOptions,
    ) -> Result<Ts2abcOutput, Ts2abcError> {
        self.backend.compile(&options)
    }
}

// ---------------------------------------------------------------------------
// SdkCommandBuilder
// ---------------------------------------------------------------------------

/// Builds the `es2abc` command-line argument list.
///
/// Used by `SdkEs2abcBackend` to construct the subprocess arguments and by
/// `Ts2abcParitySnapshot::sdk_command_args`.
///
/// # Upstream correspondence
///
/// Corresponds to the argument construction in the `SdkEs2abcBackend`
/// `input_file_arg` helper and `ts2abc.js` flags.
pub struct SdkCommandBuilder {
    es2abc_path: PathBuf,
}

impl SdkCommandBuilder {
    /// Create a builder for the given `es2abc` binary.
    pub fn new(es2abc_path: PathBuf) -> Self {
        Self { es2abc_path }
    }

    /// Build the `Command` for the given options.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to the command construction in `SdkEs2abcBackend::compile`
    /// in `driver.rs`.
    pub fn build_command(&self, options: &Ts2abcOptions) -> Command {
        let mut cmd = Command::new(&self.es2abc_path);
        cmd.arg("--target-api-version")
            .arg(options.target_api_version.to_string())
            .arg("--output")
            .arg(&options.output);
        if let Some(stage) = options
            .target_api_sub_version
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            cmd.arg("--target-api-sub-version").arg(stage);
        }
        if options.enable_annotations {
            cmd.arg("--enable-annotations");
        }
        if options.use_define_semantic {
            cmd.arg("--use-define-semantic");
        }
        if options.debug_info {
            cmd.arg("--debug-info");
        }
        if options.opt_level != 2 {
            cmd.arg("--opt-level").arg(options.opt_level.to_string());
        }
        if !options.optimize_try_catch_func {
            cmd.arg("--opt-try-catch-func=false");
        }
        if options.merge_abc {
            cmd.arg("--merge-abc");
        }
        if options.module {
            cmd.arg("--module");
        }
        if options.commonjs {
            cmd.arg("--commonjs");
        }
        if options.input_extension != "js" {
            cmd.arg("--extension").arg(&options.input_extension);
        }
        if let Some(path) = &options.compile_context_info.path {
            cmd.arg("--compile-context-info").arg(path);
        }
        cmd
    }

    /// Build the input file argument (`@filesInfo.txt` or direct path).
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `SdkEs2abcBackend::input_file_arg` in `driver.rs`.
    pub fn input_file_arg(&self, options: &Ts2abcOptions) -> Result<OsString, Ts2abcError> {
        // Use at-file syntax for multi-file or complex inputs.
        if options.output.parent().is_some() {
            let file_info_path = FileInfoManifestPath::from_output(&options.output).path();

            let mut content = FileInfoManifestText::new();
            for input in &options.inputs {
                let line = FileInfoLine::from_input(FileInfoInput {
                    path: &input.path,
                    record_name: &input.record_name,
                    source_file: input.source_file.as_deref(),
                    package_name: input.package_name.as_deref(),
                    is_commonjs: input.is_commonjs,
                    force_commonjs: options.commonjs,
                    is_shared_module: input.is_shared_module,
                    origin_source_lang: &input.origin_source_lang,
                })?;
                content.push_line(&line);
            }
            fs_err::write(&file_info_path, content.into_string())
                .map_err(|e| Ts2abcError::Io(e.into()))?;

            let mut at = OsString::from("@");
            at.push(file_info_path.as_os_str());
            return Ok(at);
        }
        if options.inputs.len() == 1 {
            Ok(options.inputs[0].path.as_os_str().to_os_string())
        } else {
            Err(Ts2abcError::InvalidPlan(
                "multiple inputs require an output path parent".to_string(),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Ts2abcDriverMigrationAdapter
// ---------------------------------------------------------------------------

/// Migration adapter wrapping the legacy `Ts2abcOptions`-based API.
///
/// Per SPEC-048, the public free functions `compile_to_abc` and
/// `compile_to_abc_with_backend` must be replaced by named types.  This
/// adapter provides the SPEC-048 target method names as associated functions
/// on a named type.  Once all callers migrate to `Ts2abcCompiler`, this
/// adapter will be deleted.
///
/// # Upstream correspondence
///
/// Replaces `compile_to_abc`/`compile_to_abc_with_backend` free functions.
pub struct Ts2abcDriverMigrationAdapter;

impl Ts2abcDriverMigrationAdapter {
    /// Compile the given options.
    ///
    /// Replaces `compile_to_abc` free function.
    pub fn compile_options(options: Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
        BytecodeBackendRunner::compile_with_default_backend(options)
    }

    /// Compile using an explicit backend.
    ///
    /// Replaces `compile_to_abc_with_backend` free function.
    pub fn compile_with_backend(
        options: Ts2abcOptions,
        backend: &dyn BytecodeBackend,
    ) -> Result<Ts2abcOutput, Ts2abcError> {
        backend.compile(&options)
    }

    /// Derive a default record name from a source path.
    ///
    /// Delegates to [`RecordNameDeriver`] for the legacy migration adapter.
    pub fn default_record_name(path: &Path) -> String {
        RecordNameDeriver::default_record_name(path)
    }
}

// ---------------------------------------------------------------------------
// Helper: plan_to_legacy_options
// ---------------------------------------------------------------------------

/// Convert a [`Ts2abcPlan`] to the legacy [`Ts2abcOptions`] format.
///
/// This is a temporary bridge during SPEC-048 migration.  Once the Rust AOT
/// backend is updated to accept `Ts2abcPlan` directly, this function will be
/// deleted.
pub(crate) fn plan_to_legacy_options(plan: &Ts2abcPlan) -> Result<Ts2abcOptions, Ts2abcError> {
    let inputs: Vec<Ts2abcInput> = plan
        .compile_units
        .iter()
        .map(|unit| {
            let path = match &unit.source {
                crate::plan::ModuleSourceInput::File { path } => path.clone(),
                crate::plan::ModuleSourceInput::Inline { virtual_path, .. } => virtual_path.clone(),
                crate::plan::ModuleSourceInput::AbcFile { path } => path.clone(),
            };
            Ts2abcInput {
                path,
                output: unit.output_path.clone(),
                cache_file: unit.cache_file.clone(),
                record_name: unit.record_name.resolve(),
                source_file: unit.source_file.clone(),
                package_name: unit.package_name.clone(),
                is_commonjs: unit.is_commonjs,
                is_shared_module: unit.is_shared_module,
                origin_source_lang: unit.origin_source_lang.clone(),
                source_map: None,
            }
        })
        .collect();

    let context = if let Some(ctx_path) = &plan.compile_context_info_path {
        crate::compile_context::CompileContextInfo::read_upstream_lenient(ctx_path)
            .unwrap_or_default()
    } else {
        crate::compile_context::CompileContextInfo::default()
    };

    Ok(Ts2abcOptions {
        inputs,
        output: plan.output.abc_output.clone(),
        target_api_version: plan.target_api_version,
        target_api_sub_version: plan.target_api_sub_version.clone(),
        enable_annotations: plan.enable_annotations,
        use_define_semantic: plan.use_define_semantic,
        debug_info: plan.debug_info,
        opt_level: plan.opt_level,
        optimize_try_catch_func: plan.optimize_try_catch_func,
        merge_abc: plan.merge_abc,
        module: plan.module_mode,
        commonjs: false,
        input_extension: "js".to_string(),
        module_record_field_name: None,
        cache_file: plan.output.cache_file.clone(),
        output_proto: plan.output.proto_output.clone(),
        compile_context_info: context,
        enable_abc_input: plan.enable_abc_input,
        dump_deps_info: plan.dump_deps_info,
        remove_redundant_file: plan.remove_redundant_file,
        npm_module_entry_list: plan.npm_module_entry_list.clone(),
    })
}
