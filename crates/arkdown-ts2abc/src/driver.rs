use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_abc::{
    AbcCompileOptions, AbcCompiler, AbcDependencyContext, AbcInputFile, AbcModule,
    AbcProgramCacheInput,
};

use crate::compile_context::CompileContextInfo;
use crate::error::Ts2abcError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ts2abcInput {
    pub path: PathBuf,
    /// Per-input ABC output path used by upstream non-merge `OutputFiles()`.
    pub output: Option<PathBuf>,
    /// Per-input upstream ProgramCache `.protoBin` file from `--cache-file`.
    pub cache_file: Option<PathBuf>,
    pub record_name: String,
    pub source_file: Option<String>,
    pub package_name: Option<String>,
    pub is_commonjs: bool,
    pub is_shared_module: bool,
    pub origin_source_lang: String,
    pub source_map: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ts2abcOptions {
    pub inputs: Vec<Ts2abcInput>,
    pub output: PathBuf,
    pub target_api_version: u32,
    pub target_api_sub_version: Option<String>,
    pub enable_annotations: bool,
    pub use_define_semantic: bool,
    pub debug_info: bool,
    pub opt_level: u8,
    pub optimize_try_catch_func: bool,
    pub merge_abc: bool,
    pub module: bool,
    pub commonjs: bool,
    pub input_extension: String,
    pub module_record_field_name: Option<String>,
    pub cache_file: Option<PathBuf>,
    pub output_proto: Option<PathBuf>,
    pub compile_context_info: CompileContextInfo,
    pub enable_abc_input: bool,
    pub dump_deps_info: bool,
    pub remove_redundant_file: bool,
    pub npm_module_entry_list: Option<PathBuf>,
}

impl Ts2abcOptions {
    /// Compile this set of inputs using the pure-Rust ABC backend.
    pub fn compile(self) -> Result<Ts2abcOutput, Ts2abcError> {
        ArkdownAbcBackend.compile(&self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ts2abcOutput {
    pub abc_path: PathBuf,
    pub abc_paths: Vec<PathBuf>,
    pub proto_path: Option<PathBuf>,
    pub compiled_records: Vec<String>,
    pub target_api_version: u32,
    pub debug_info: bool,
}

pub trait BytecodeBackend {
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ArkdownAbcBackend;

impl BytecodeBackend for ArkdownAbcBackend {
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
        if options.inputs.is_empty() {
            return Err(Ts2abcError::MissingInput);
        }
        if options
            .inputs
            .iter()
            .any(|input| is_abc_input(&input.path, &options.input_extension))
        {
            return self.compile_abc_inputs(options);
        }

        if !options.merge_abc && options.inputs.iter().any(|input| input.output.is_some()) {
            return self.compile_non_merge_outputs(options);
        }

        let (modules, cache_inputs) = collect_modules_and_cache_inputs(options)?;

        let compiler = AbcCompiler::new(AbcCompileOptions {
            debug_info: options.debug_info,
            opt_level: options.opt_level,
            optimize_try_catch_func: options.optimize_try_catch_func,
            target_api_version: options.target_api_version,
            target_api_sub_version: options.target_api_sub_version.clone(),
            enable_annotations: options.enable_annotations,
            use_define_semantic: options.use_define_semantic,
            merge_abc: options.merge_abc,
            module: options.module,
            module_record_field_name: options.module_record_field_name.clone(),
            npm_entries: options.npm_module_entry_list.clone(),
            dependency_context: dependency_context(options),
            remove_redundant_file: options.remove_redundant_file,
            dump_deps_info: options.dump_deps_info,
        });

        if let Some(output_proto) = &options.output_proto {
            if options.inputs.len() == 1 {
                let artifact =
                    compiler.compile_program_proto(modules, output_proto, cache_inputs)?;
                return Ok(Ts2abcOutput {
                    abc_path: output_proto.clone(),
                    abc_paths: Vec::new(),
                    proto_path: Some(artifact.path),
                    compiled_records: options
                        .inputs
                        .iter()
                        .map(|input| input.record_name.clone())
                        .collect(),
                    target_api_version: artifact.api_version,
                    debug_info: artifact.has_debug_info,
                });
            }
        }

        let artifact =
            compiler.compile_with_program_cache(modules, &options.output, cache_inputs)?;
        let abc_path = artifact.path;
        Ok(Ts2abcOutput {
            abc_paths: vec![abc_path.clone()],
            abc_path,
            proto_path: None,
            compiled_records: options
                .inputs
                .iter()
                .map(|input| input.record_name.clone())
                .collect(),
            target_api_version: artifact.api_version,
            debug_info: artifact.has_debug_info,
        })
    }
}

impl ArkdownAbcBackend {
    fn compile_abc_inputs(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
        for input in &options.inputs {
            reject_unsupported_abc_input(input, options)?;
        }
        if !options.merge_abc && options.inputs.len() > 1 {
            return Err(Ts2abcError::UnsupportedFeature {
                flag: "--enable-abc-input".to_string(),
                reason: "multiple ABC inputs require merge-abc output in the Rust backend"
                    .to_string(),
            });
        }

        let compiler = AbcCompiler::new(AbcCompileOptions {
            debug_info: options.debug_info,
            opt_level: options.opt_level,
            optimize_try_catch_func: options.optimize_try_catch_func,
            target_api_version: options.target_api_version,
            target_api_sub_version: options.target_api_sub_version.clone(),
            enable_annotations: options.enable_annotations,
            use_define_semantic: options.use_define_semantic,
            merge_abc: options.merge_abc,
            module: options.module,
            module_record_field_name: options.module_record_field_name.clone(),
            npm_entries: options.npm_module_entry_list.clone(),
            dependency_context: dependency_context(options),
            remove_redundant_file: options.remove_redundant_file,
            dump_deps_info: options.dump_deps_info,
        });

        if let Some(output_proto) = &options.output_proto {
            let artifact = compiler
                .compile_abc_input_program_proto(load_abc_inputs(options)?, output_proto)?;
            return Ok(Ts2abcOutput {
                abc_path: output_proto.clone(),
                abc_paths: Vec::new(),
                proto_path: Some(artifact.path),
                compiled_records: options
                    .inputs
                    .iter()
                    .map(|input| input.record_name.clone())
                    .collect(),
                target_api_version: artifact.api_version,
                debug_info: artifact.has_debug_info,
            });
        }

        let artifact = compiler.compile_abc_inputs(load_abc_inputs(options)?, &options.output)?;
        let abc_path = artifact.path;
        Ok(Ts2abcOutput {
            abc_paths: vec![abc_path.clone()],
            abc_path,
            proto_path: None,
            compiled_records: options
                .inputs
                .iter()
                .map(|input| input.record_name.clone())
                .collect(),
            target_api_version: artifact.api_version,
            debug_info: artifact.has_debug_info,
        })
    }

    fn compile_non_merge_outputs(
        &self,
        options: &Ts2abcOptions,
    ) -> Result<Ts2abcOutput, Ts2abcError> {
        let compiler = AbcCompiler::new(AbcCompileOptions {
            debug_info: options.debug_info,
            opt_level: options.opt_level,
            optimize_try_catch_func: options.optimize_try_catch_func,
            target_api_version: options.target_api_version,
            target_api_sub_version: options.target_api_sub_version.clone(),
            enable_annotations: options.enable_annotations,
            use_define_semantic: options.use_define_semantic,
            merge_abc: false,
            module: options.module,
            module_record_field_name: options.module_record_field_name.clone(),
            npm_entries: None,
            dependency_context: dependency_context(options),
            remove_redundant_file: options.remove_redundant_file,
            dump_deps_info: options.dump_deps_info,
        });

        let mut abc_paths = Vec::with_capacity(options.inputs.len());
        let mut compiled_records = Vec::with_capacity(options.inputs.len());
        let mut has_debug_info = false;
        for input in &options.inputs {
            reject_unsupported_abc_input(input, options)?;
            let Some(output) = &input.output else {
                return Err(Ts2abcError::UnsupportedFeature {
                    flag: "non-merge outputFiles".to_string(),
                    reason: format!("missing per-input output file for {}", input.path.display()),
                });
            };
            if !input.path.exists() {
                return Err(Ts2abcError::MissingSource(input.path.clone()));
            }
            let source = fs_err::read_to_string(&input.path)?;
            let cache_inputs =
                vec![input
                    .cache_file
                    .as_ref()
                    .map(|cache_file| AbcProgramCacheInput {
                        file_name: input.path.to_string_lossy().into_owned(),
                        cache_file: cache_file.clone(),
                    })];
            let artifact = compiler.compile_with_program_cache(
                vec![AbcModule {
                    record_name: input.record_name.clone(),
                    source,
                    source_map: input.source_map.clone(),
                    source_file: input.source_file.clone(),
                    package_name: input.package_name.clone(),
                    is_commonjs: input.is_commonjs,
                    is_shared_module: input.is_shared_module,
                    origin_source_lang: input.origin_source_lang.clone(),
                }],
                output,
                cache_inputs,
            )?;
            has_debug_info |= artifact.has_debug_info;
            abc_paths.push(artifact.path);
            compiled_records.push(input.record_name.clone());
        }

        Ok(Ts2abcOutput {
            abc_path: abc_paths
                .first()
                .cloned()
                .unwrap_or_else(|| options.output.clone()),
            abc_paths,
            proto_path: None,
            compiled_records,
            target_api_version: options.target_api_version,
            debug_info: has_debug_info,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SdkEs2abcBackend {
    es2abc_path: PathBuf,
    input_file_list: Option<PathBuf>,
}

impl SdkEs2abcBackend {
    pub fn new(es2abc_path: PathBuf) -> Self {
        Self {
            es2abc_path,
            input_file_list: None,
        }
    }

    pub fn with_input_file_list(mut self, input_file_list: Option<PathBuf>) -> Self {
        self.input_file_list = input_file_list;
        self
    }
}

impl BytecodeBackend for SdkEs2abcBackend {
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
        if options.inputs.is_empty() {
            return Err(Ts2abcError::MissingInput);
        }
        if !self.es2abc_path.exists() {
            return Err(Ts2abcError::SdkCompilerNotFound(self.es2abc_path.clone()));
        }
        if let Some(parent) = options.output.parent() {
            fs_err::create_dir_all(parent)?;
        }

        let input_arg = self.input_file_arg(options)?;
        let mut command = Command::new(&self.es2abc_path);
        command
            .arg("--target-api-version")
            .arg(options.target_api_version.to_string())
            .arg("--output")
            .arg(&options.output);

        if let Some(stage) = non_empty_str(options.target_api_sub_version.as_deref()) {
            command.arg("--target-api-sub-version").arg(stage);
        }
        if options.enable_annotations {
            command.arg("--enable-annotations");
        }
        if options.use_define_semantic {
            command.arg("--use-define-semantic");
        }
        if options.debug_info {
            command.arg("--debug-info");
        }
        if options.opt_level != 2 {
            command
                .arg("--opt-level")
                .arg(options.opt_level.to_string());
        }
        if !options.optimize_try_catch_func {
            command.arg("--opt-try-catch-func=false");
        }
        if options.merge_abc {
            command.arg("--merge-abc");
        }
        if options.module {
            command.arg("--module");
        }
        if options.commonjs {
            command.arg("--commonjs");
        }
        if options.input_extension != "js" {
            command.arg("--extension").arg(&options.input_extension);
        }
        if let Some(field_name) = non_empty_str(options.module_record_field_name.as_deref()) {
            command.arg("--module-record-field-name").arg(field_name);
        }
        if let Some(cache_file) = &options.cache_file {
            command.arg("--cache-file").arg(cache_file_arg(cache_file));
        }
        if let Some(output_proto) = &options.output_proto {
            command.arg("--outputProto").arg(output_proto);
        }
        if options.enable_abc_input {
            command.arg("--enable-abc-input");
        }
        if options.dump_deps_info {
            command.arg("--dump-deps-info");
        }
        if options.remove_redundant_file {
            command.arg("--remove-redundant-file");
        }
        if let Some(npm_entries) = &options.npm_module_entry_list {
            command.arg("--npm-module-entry-list").arg(npm_entries);
        }
        command.arg(input_arg);

        let output = command
            .output()
            .map_err(|source| Ts2abcError::SdkCompilerIo {
                program: self.es2abc_path.display().to_string(),
                source,
            })?;
        if !output.status.success() {
            return Err(Ts2abcError::SdkCompilerFailed {
                status: output.status.to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let abc_path = options
            .output_proto
            .clone()
            .unwrap_or_else(|| options.output.clone());
        let abc_paths = if options.output_proto.is_some() {
            Vec::new()
        } else {
            vec![abc_path.clone()]
        };
        Ok(Ts2abcOutput {
            abc_paths,
            abc_path,
            proto_path: options.output_proto.clone(),
            compiled_records: options
                .inputs
                .iter()
                .map(|input| input.record_name.clone())
                .collect(),
            target_api_version: options.target_api_version,
            debug_info: options.debug_info,
        })
    }
}

impl SdkEs2abcBackend {
    fn input_file_arg(&self, options: &Ts2abcOptions) -> Result<OsString, Ts2abcError> {
        if let Some(input_file_list) = &self.input_file_list {
            return Ok(at_file_arg(input_file_list));
        }

        if options.inputs.len() == 1 && !needs_file_info(&options.inputs[0], options) {
            return Ok(options.inputs[0].path.as_os_str().to_os_string());
        }

        let file_list = generated_file_info_path(&options.output);
        let mut content = String::new();
        for input in &options.inputs {
            content.push_str(&file_info_line(input, options));
            content.push('\n');
        }
        fs_err::write(&file_list, content)?;
        Ok(at_file_arg(&file_list))
    }
}

fn load_abc_inputs(options: &Ts2abcOptions) -> Result<Vec<AbcInputFile>, Ts2abcError> {
    let mut inputs = Vec::with_capacity(options.inputs.len());
    for input in &options.inputs {
        if !input.path.exists() {
            return Err(Ts2abcError::MissingSource(input.path.clone()));
        }
        inputs.push(AbcInputFile {
            file_name: input.path.to_string_lossy().into_owned(),
            record_name: input.record_name.clone(),
            source_file: input.source_file.clone().unwrap_or_default(),
            package_name: input.package_name.clone().unwrap_or_default(),
            cache_file: input.cache_file.clone(),
            bytes: fs_err::read(&input.path)?,
        });
    }
    Ok(inputs)
}

pub fn compile_to_abc(options: Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
    options.compile()
}

pub fn compile_to_abc_with_backend(
    options: Ts2abcOptions,
    backend: &dyn BytecodeBackend,
) -> Result<Ts2abcOutput, Ts2abcError> {
    backend.compile(&options)
}

fn dependency_context(options: &Ts2abcOptions) -> Option<AbcDependencyContext> {
    (options.enable_abc_input && options.compile_context_info.needs_dependency_relation()).then(
        || AbcDependencyContext {
            compile_entries: options.compile_context_info.compile_entries.clone(),
            external_pkg_names: options.compile_context_info.hsp_pkg_names.clone(),
        },
    )
}

fn reject_unsupported_abc_input(
    input: &Ts2abcInput,
    options: &Ts2abcOptions,
) -> Result<(), Ts2abcError> {
    if !is_abc_input(&input.path, &options.input_extension) {
        return Ok(());
    }
    if !options.enable_abc_input {
        return Err(Ts2abcError::UnsupportedFeature {
            flag: "--enable-abc-input".to_string(),
            reason: format!(
                "input abc file {} requires --enable-abc-input",
                input.path.display()
            ),
        });
    }
    Ok(())
}

fn is_abc_input(path: &Path, input_extension: &str) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| matches!(*extension, "js" | "ts" | "as" | "abc"))
        .unwrap_or(input_extension);
    extension == "abc"
}

fn collect_modules_and_cache_inputs(
    options: &Ts2abcOptions,
) -> Result<(Vec<AbcModule>, Vec<Option<AbcProgramCacheInput>>), Ts2abcError> {
    let mut modules = Vec::with_capacity(options.inputs.len());
    let mut cache_inputs = Vec::with_capacity(options.inputs.len());
    for input in &options.inputs {
        if !input.path.exists() {
            return Err(Ts2abcError::MissingSource(input.path.clone()));
        }
        let source = fs_err::read_to_string(&input.path)?;
        modules.push(AbcModule {
            record_name: input.record_name.clone(),
            source,
            source_map: input.source_map.clone(),
            source_file: input.source_file.clone(),
            package_name: input.package_name.clone(),
            is_commonjs: input.is_commonjs,
            is_shared_module: input.is_shared_module,
            origin_source_lang: input.origin_source_lang.clone(),
        });
        cache_inputs.push(
            input
                .cache_file
                .as_ref()
                .map(|cache_file| AbcProgramCacheInput {
                    file_name: input.path.to_string_lossy().into_owned(),
                    cache_file: cache_file.clone(),
                }),
        );
    }
    Ok((modules, cache_inputs))
}

pub(crate) fn default_record_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("record")
        .to_string()
}

fn needs_file_info(input: &Ts2abcInput, options: &Ts2abcOptions) -> bool {
    options.merge_abc
        || !input.record_name.is_empty()
        || input.source_file.is_some()
        || input.package_name.is_some()
        || input.is_commonjs
        || input.is_shared_module
        || !input.origin_source_lang.is_empty()
}

fn file_info_line(input: &Ts2abcInput, options: &Ts2abcOptions) -> String {
    let module_type = if input.is_commonjs || options.commonjs {
        "commonjs"
    } else {
        "esm"
    };
    let source_file = input
        .source_file
        .clone()
        .unwrap_or_else(|| input.path.to_string_lossy().into_owned());
    let package_name = input.package_name.clone().unwrap_or_default();
    format!(
        "{};{};{};{};{};{};{}",
        input.path.to_string_lossy(),
        input.record_name,
        module_type,
        source_file,
        package_name,
        input.is_shared_module,
        input.origin_source_lang
    )
}

fn generated_file_info_path(output: &Path) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("modules");
    parent.join(format!("{stem}.filesInfo.txt"))
}

fn at_file_arg(path: &Path) -> OsString {
    let mut value = OsString::from("@");
    value.push(path.as_os_str());
    value
}

fn cache_file_arg(path: &Path) -> OsString {
    let raw = path.as_os_str().to_string_lossy();
    if raw.starts_with('@') {
        return path.as_os_str().to_os_string();
    }
    if path.file_name().and_then(OsStr::to_str) == Some("modules.cache") {
        return at_file_arg(path);
    }
    path.as_os_str().to_os_string()
}

fn non_empty_str(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
