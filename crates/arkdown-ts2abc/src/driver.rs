use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::compile_context::CompileContextInfo;
use crate::error::Ts2abcError;
use crate::file_info::{FileInfoInput, FileInfoLine, FileInfoManifestPath, FileInfoManifestText};

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
    /// Compile this set of inputs.
    ///
    /// `Ts2abcOptions` does not carry the SDK binary path, so callers must use
    /// [`SdkEs2abcBackend`] or the typed [`crate::Ts2abcCompiler`] path.
    pub fn compile(self) -> Result<Ts2abcOutput, Ts2abcError> {
        Err(Ts2abcError::MissingValue(
            "sdk es2abc path is required".to_string(),
        ))
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

struct Es2abcPathArgument<'a> {
    path: &'a Path,
}

impl<'a> Es2abcPathArgument<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn at_prefixed(&self) -> OsString {
        let mut value = OsString::from("@");
        value.push(self.path.as_os_str());
        value
    }

    fn cache_file(&self) -> OsString {
        if self
            .path
            .as_os_str()
            .to_str()
            .is_some_and(|raw| raw.starts_with('@'))
        {
            return self.path.as_os_str().to_os_string();
        }
        if self.path.file_name().and_then(OsStr::to_str) == Some("modules.cache") {
            return self.at_prefixed();
        }
        self.path.as_os_str().to_os_string()
    }
}

struct SdkFileInfoManifest<'a> {
    output: &'a Path,
    inputs: &'a [Ts2abcInput],
    commonjs: bool,
}

impl<'a> SdkFileInfoManifest<'a> {
    fn new(options: &'a Ts2abcOptions) -> Self {
        Self {
            output: &options.output,
            inputs: &options.inputs,
            commonjs: options.commonjs,
        }
    }

    fn path(&self) -> PathBuf {
        FileInfoManifestPath::from_output(self.output).path()
    }

    fn write(&self) -> Result<PathBuf, Ts2abcError> {
        let file_list = self.path();
        let mut content = FileInfoManifestText::new();
        for input in self.inputs {
            content.push_line(&self.line_for(input)?);
        }
        fs_err::write(&file_list, content.into_string())?;
        Ok(file_list)
    }

    fn line_for(&self, input: &Ts2abcInput) -> Result<FileInfoLine, Ts2abcError> {
        FileInfoLine::from_input(FileInfoInput {
            path: &input.path,
            record_name: &input.record_name,
            source_file: input.source_file.as_deref(),
            package_name: input.package_name.as_deref(),
            is_commonjs: input.is_commonjs,
            force_commonjs: self.commonjs,
            is_shared_module: input.is_shared_module,
            origin_source_lang: &input.origin_source_lang,
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

        let mut command = self.build_command(options)?;

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
    fn build_command(&self, options: &Ts2abcOptions) -> Result<Command, Ts2abcError> {
        let input_arg = self.input_file_arg(options)?;
        let mut command = Command::new(&self.es2abc_path);
        if self.is_hvigor_module_mode(options) {
            self.append_hvigor_module_mode_args(&mut command, options, input_arg);
        } else {
            self.append_direct_es2abc_args(&mut command, options, input_arg);
        }
        Ok(command)
    }

    fn is_hvigor_module_mode(&self, options: &Ts2abcOptions) -> bool {
        self.input_file_list.is_some() && options.merge_abc
    }

    fn append_hvigor_module_mode_args(
        &self,
        command: &mut Command,
        options: &Ts2abcOptions,
        input_arg: OsString,
    ) {
        if options.debug_info {
            command.arg("--debug-info");
        }

        command.arg(input_arg);

        if let Some(npm_entries) = &options.npm_module_entry_list {
            command.arg("--npm-module-entry-list").arg(npm_entries);
        }

        command
            .arg("--output")
            .arg(&options.output)
            .arg("--file-threads")
            .arg(self.file_threads().to_string())
            .arg("--merge-abc")
            .arg(format!(
                "--target-api-version={}",
                options.target_api_version
            ));

        if let Some(stage) = non_empty_str(options.target_api_sub_version.as_deref()) {
            command.arg(format!("--target-api-sub-version={stage}"));
        }
        if let Some(path) = &options.compile_context_info.path {
            command.arg("--compile-context-info").arg(path);
        }
        if options.enable_abc_input {
            command.arg("--enable-abc-input");
        }
        if options.remove_redundant_file {
            command.arg("--remove-redundant-file");
        }
        if !options.optimize_try_catch_func {
            command.arg("--opt-try-catch-func=false");
        }
        if options.enable_annotations {
            command.arg("--enable-annotations");
        }
        command.arg("--enable-release-column");
        if let Some(cache_file) = &options.cache_file {
            command
                .arg("--cache-file")
                .arg(Es2abcPathArgument::new(cache_file).cache_file());
        }
        if options.dump_deps_info {
            command.arg("--dump-deps-info");
        }
    }

    fn append_direct_es2abc_args(
        &self,
        command: &mut Command,
        options: &Ts2abcOptions,
        input_arg: OsString,
    ) {
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
            command
                .arg("--cache-file")
                .arg(Es2abcPathArgument::new(cache_file).cache_file());
        }
        if let Some(output_proto) = &options.output_proto {
            command.arg("--outputProto").arg(output_proto);
        }
        if let Some(path) = &options.compile_context_info.path {
            command.arg("--compile-context-info").arg(path);
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
    }

    fn file_threads(&self) -> usize {
        std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(16)
    }

    fn input_file_arg(&self, options: &Ts2abcOptions) -> Result<OsString, Ts2abcError> {
        if let Some(input_file_list) = &self.input_file_list {
            return Ok(Es2abcPathArgument::new(input_file_list).at_prefixed());
        }

        if options.inputs.len() == 1 && !needs_file_info(&options.inputs[0], options) {
            return Ok(options.inputs[0].path.as_os_str().to_os_string());
        }

        let file_list = SdkFileInfoManifest::new(options).write()?;
        Ok(Es2abcPathArgument::new(&file_list).at_prefixed())
    }
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

fn needs_file_info(input: &Ts2abcInput, options: &Ts2abcOptions) -> bool {
    options.merge_abc
        || !input.record_name.is_empty()
        || input.source_file.is_some()
        || input.package_name.is_some()
        || input.is_commonjs
        || input.is_shared_module
        || !input.origin_source_lang.is_empty()
}

fn non_empty_str(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
