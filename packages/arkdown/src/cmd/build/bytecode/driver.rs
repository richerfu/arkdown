use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use arkdown_session::SdkEs2abcOptionSnapshot;

use crate::cmd::build::bytecode::compile_context::CompileContextInfo;
use crate::cmd::build::bytecode::error::Ts2abcError;
use crate::cmd::build::bytecode::file_info::{
  FileInfoInput, FileInfoLine, FileInfoManifestPath, FileInfoManifestText,
};

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
  /// es2abc `--branch-elimination` (release, non-debuggable, no bytecode
  /// obfuscation — mirrors `module_mode.js` command construction).
  pub branch_elimination: bool,
  /// es2abc `--transform-lib` AOP transform library path when configured.
  pub transform_lib: Option<PathBuf>,
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

pub(crate) trait BytecodeBackend {
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
pub(crate) struct SdkEs2abcBackend {
  es2abc_path: PathBuf,
  es2abc_options: Arc<SdkEs2abcOptionSnapshot>,
  input_file_list: Option<PathBuf>,
}

impl SdkEs2abcBackend {
  pub(crate) fn new(es2abc_path: PathBuf, es2abc_options: Arc<SdkEs2abcOptionSnapshot>) -> Self {
    Self {
      es2abc_path,
      es2abc_options,
      input_file_list: None,
    }
  }

  pub(crate) fn with_input_file_list(mut self, input_file_list: Option<PathBuf>) -> Self {
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
    self.validate_command_options(&command)?;
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
    // `abstract-compile-node.js::declareInputs` only sets projectConfig
    // branchElimination for non-debuggable builds; mirror that here so a
    // debug build never passes the flag even if a caller misconfigures it.
    if options.branch_elimination && !options.debug_info {
      command.arg("--branch-elimination");
    }
    if let Some(transform_lib) = &options.transform_lib {
      command.arg("--transform-lib").arg(transform_lib);
    }
    if options.enable_annotations {
      command.arg("--enable-annotations");
    }
    command.arg("--enable-release-column");
    // Hvigor loads `genAbc()` from the selected SDK's `compile_plugin.js`.
    // API 20/21/23 module_mode.js does not emit this option, while API 24
    // adds it together with compiler support. The actual tool inventory is
    // the parity boundary, rather than a guessed API-version threshold.
    if self.es2abc_options.supports("--enable-callable-name") {
      command.arg("--enable-callable-name");
    }
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

  fn validate_command_options(&self, command: &Command) -> Result<(), Ts2abcError> {
    if !self.es2abc_options.is_available() {
      return Err(Ts2abcError::SdkCompilerOptionProbeFailed {
        program: self.es2abc_path.clone(),
        reason: self
          .es2abc_options
          .probe_failure()
          .unwrap_or("es2abc --help returned no option inventory")
          .to_string(),
      });
    }

    let mut unsupported = command
      .get_args()
      .filter_map(|argument| argument.to_str())
      .filter_map(Es2abcCommandOption::new)
      .filter(|option| !self.es2abc_options.supports(option.name()))
      .map(|option| option.name().to_string())
      .collect::<Vec<_>>();
    unsupported.sort_unstable();
    unsupported.dedup();
    if unsupported.is_empty() {
      return Ok(());
    }
    Err(Ts2abcError::UnsupportedSdkCompilerOptions {
      program: self.es2abc_path.clone(),
      options: unsupported.join(", "),
    })
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

struct Es2abcCommandOption<'a> {
  argument: &'a str,
  name_end: usize,
}

impl<'a> Es2abcCommandOption<'a> {
  fn new(argument: &'a str) -> Option<Self> {
    let name = argument.strip_prefix("--")?;
    let name_end = name
      .find(|character: char| !character.is_ascii_alphanumeric() && character != '-')
      .unwrap_or(name.len());
    (name_end > 0).then_some(Self { argument, name_end })
  }

  fn name(&self) -> &str {
    &self.argument[..self.name_end + 2]
  }
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

#[cfg(test)]
mod tests {
  use super::*;

  const API_20_TO_23_OPTIONS: &[&str] = &[
    "--branch-elimination",
    "--cache-file",
    "--commonjs",
    "--compile-context-info",
    "--debug-info",
    "--dump-deps-info",
    "--enable-abc-input",
    "--enable-annotations",
    "--enable-release-column",
    "--extension",
    "--file-threads",
    "--merge-abc",
    "--module",
    "--module-record-field-name",
    "--npm-module-entry-list",
    "--opt-level",
    "--opt-try-catch-func",
    "--output",
    "--outputProto",
    "--remove-redundant-file",
    "--target-api-sub-version",
    "--target-api-version",
    "--transform-lib",
    "--use-define-semantic",
  ];

  fn sdk_options(enable_callable_name: bool) -> Arc<SdkEs2abcOptionSnapshot> {
    let mut options = API_20_TO_23_OPTIONS.to_vec();
    if enable_callable_name {
      options.push("--enable-callable-name");
    }
    Arc::new(SdkEs2abcOptionSnapshot::from_options(options))
  }

  fn module_mode_options() -> Ts2abcOptions {
    Ts2abcOptions {
      inputs: vec![Ts2abcInput {
        path: PathBuf::from("/tmp/entry.js"),
        output: None,
        cache_file: None,
        record_name: "entry".to_string(),
        source_file: Some("entry.js".to_string()),
        package_name: Some("entry".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "js".to_string(),
        source_map: None,
      }],
      output: PathBuf::from("/tmp/modules.abc"),
      target_api_version: 23,
      target_api_sub_version: None,
      enable_annotations: false,
      use_define_semantic: false,
      debug_info: false,
      opt_level: 2,
      optimize_try_catch_func: true,
      merge_abc: true,
      module: true,
      commonjs: false,
      input_extension: "js".to_string(),
      module_record_field_name: None,
      cache_file: None,
      output_proto: None,
      compile_context_info: CompileContextInfo::default(),
      enable_abc_input: false,
      dump_deps_info: false,
      remove_redundant_file: false,
      npm_module_entry_list: None,
      branch_elimination: false,
      transform_lib: None,
    }
  }

  fn args_for_sdk(options: &Ts2abcOptions, enable_callable_name: bool) -> Vec<String> {
    let backend = SdkEs2abcBackend::new(
      PathBuf::from("/fake/es2abc"),
      sdk_options(enable_callable_name),
    )
    .with_input_file_list(Some(PathBuf::from("/tmp/filesInfo.txt")));
    let command = backend.build_command(options).expect("build command");
    command
      .get_args()
      .map(|arg| arg.to_string_lossy().into_owned())
      .collect()
  }

  fn args_for(options: &Ts2abcOptions) -> Vec<String> {
    args_for_sdk(options, true)
  }

  /// API 20/21/23 SDK `module_mode.js` and `es2abc --help` both omit the
  /// callable-name option; API 24 adds it at both boundaries.
  #[test]
  fn callable_name_matches_selected_sdk_capability() {
    let options = module_mode_options();
    let api_23_args = args_for_sdk(&options, false);
    assert!(
      !api_23_args
        .iter()
        .any(|arg| arg == "--enable-callable-name"),
      "API 23 must not receive the API 24 option: {api_23_args:?}"
    );

    let api_24_args = args_for_sdk(&options, true);
    assert!(
      api_24_args
        .iter()
        .any(|arg| arg == "--enable-callable-name"),
      "API 24 must preserve its SDK loader option: {api_24_args:?}"
    );
  }

  #[test]
  fn rejects_every_unsupported_planned_option_before_spawn() {
    let options = module_mode_options();
    let backend = SdkEs2abcBackend::new(
      PathBuf::from("/fake/es2abc"),
      Arc::new(SdkEs2abcOptionSnapshot::from_options(["--output"])),
    )
    .with_input_file_list(Some(PathBuf::from("/tmp/filesInfo.txt")));
    let error = backend.build_command(&options).unwrap_err();
    let Ts2abcError::UnsupportedSdkCompilerOptions { options, .. } = error else {
      panic!("expected unsupported-option diagnostic, got {error}");
    };
    assert!(options.contains("--enable-release-column"), "{options}");
    assert!(options.contains("--file-threads"), "{options}");
    assert!(options.contains("--merge-abc"), "{options}");
    assert!(options.contains("--target-api-version"), "{options}");
  }

  /// `--branch-elimination` is emitted only for release builds without
  /// bytecode obfuscation (mirrors `module_mode.js` es2abc construction).
  #[test]
  fn branch_elimination_flag_is_conditional() {
    let base = module_mode_options();
    let args = args_for(&base);
    assert!(
      !args.iter().any(|arg| arg == "--branch-elimination"),
      "branch-elimination must be absent by default; args: {args:?}"
    );

    let mut enabled = module_mode_options();
    enabled.branch_elimination = true;
    let args = args_for(&enabled);
    assert!(
      args.iter().any(|arg| arg == "--branch-elimination"),
      "branch-elimination must be passed when enabled; args: {args:?}"
    );

    // Debug builds never pass the flag even when configured.
    let mut debug = module_mode_options();
    debug.branch_elimination = true;
    debug.debug_info = true;
    let args = args_for(&debug);
    assert!(
      !args.iter().any(|arg| arg == "--branch-elimination"),
      "debug builds must not pass branch-elimination; args: {args:?}"
    );
  }

  /// `--transform-lib <path>` is emitted when `arkOptions.transformLib` is set.
  #[test]
  fn transform_lib_flag_is_conditional() {
    let base = module_mode_options();
    let args = args_for(&base);
    assert!(
      !args.iter().any(|arg| arg == "--transform-lib"),
      "transform-lib must be absent by default; args: {args:?}"
    );

    let mut configured = module_mode_options();
    configured.transform_lib = Some(PathBuf::from("/tmp/aop-lib.js"));
    let args = args_for(&configured);
    let index = args
      .iter()
      .position(|arg| arg == "--transform-lib")
      .expect("transform-lib flag present");
    assert_eq!(
      args.get(index + 1).map(String::as_str),
      Some("/tmp/aop-lib.js"),
      "transform-lib must carry the configured path; args: {args:?}"
    );
  }
}
