//! Typed ts2abc compiler entry point.
//!
//! `Ts2abcCompiler` is the production owner for executing a [`Ts2abcPlan`].
//! It validates the selected backend, writes SDK handoff manifests owned by the
//! plan, and dispatches to the backend implementation.

use std::path::{Path, PathBuf};

use arkdown_core::{HvigorModuleSourceFile, HvigorNpmEntry};
use arkdown_path::lexical_normalize;
use rustc_hash::FxHashSet;

use crate::cmd::build::bytecode::backend::{BytecodeBackend, SdkEs2abcBackend};
use crate::cmd::build::bytecode::compile_context::CompileContextInfo;
use crate::cmd::build::bytecode::driver::{Ts2abcInput, Ts2abcOptions};
use crate::cmd::build::bytecode::error::Ts2abcError;
use crate::cmd::build::bytecode::file_info::{
  CacheFileManifestWriter, FileInfoColumnPolicy, FileInfoLine, FileInfoManifestText,
};
use crate::cmd::build::bytecode::plan::{
  CompileUnitPlan, ModuleSourceInput, RecordNamePlan, Ts2abcOutputPlan, Ts2abcPlan,
  Ts2abcPlanBuilder,
};
use crate::cmd::build::bytecode::Ts2abcOutput;

/// Executes typed ts2abc plans.
#[derive(Debug, Clone, Default)]
pub struct Ts2abcCompiler {
  sdk_es2abc_path: Option<PathBuf>,
}

/// Backend selection for Rolldown-backed module-source bytecode compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolldownBytecodeBackendChoice {
  Auto,
  Sdk,
}

/// Input produced by the Rolldown bundling stage for bytecode compilation.
#[derive(Debug, Clone)]
pub struct RolldownBytecodeCompileInput {
  pub module_source_files: Vec<HvigorModuleSourceFile>,
  pub external_module_requests: Vec<String>,
  pub npm_entries: Vec<HvigorNpmEntry>,
  pub modules_abc_path: PathBuf,
  pub cache_dir: PathBuf,
  pub cache_file: Option<PathBuf>,
  pub abc_input_files: Vec<BytecodeAbcInput>,
  pub compile_context_info_path: Option<PathBuf>,
  /// Whether the current output module is a ByteCode HAR.
  pub byte_code_har: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeAbcInput {
  pub path: PathBuf,
  pub package_name: String,
  pub cache_file: PathBuf,
}

impl BytecodeAbcInput {
  /// Build the cache path used by SDK `ModuleMode.genAbcCacheFilePath()`:
  /// preserve the ABC path relative to `projectTopDir` below CompileArkTS'
  /// cache directory and replace `.abc` with `.protoBin`.
  pub fn from_project_cache(
    path: PathBuf,
    package_name: impl Into<String>,
    project_root: &Path,
    cache_dir: &Path,
  ) -> Result<Self, Ts2abcError> {
    let path = lexical_normalize(&path);
    let project_root = lexical_normalize(project_root);
    let relative = path.strip_prefix(&project_root).map_err(|_| {
      Ts2abcError::InvalidPlan(format!(
        "ByteCode HAR ABC '{}' is outside projectTopDir '{}'",
        path.display(),
        project_root.display()
      ))
    })?;
    let cache_file = cache_dir.join(relative).with_extension("protoBin");
    Ok(Self {
      path,
      package_name: package_name.into(),
      cache_file,
    })
  }
}

/// Compiles Rolldown output chunks through the typed ts2abc backend pipeline.
#[derive(Debug, Clone)]
pub struct RolldownBytecodeCompiler {
  backend: RolldownBytecodeBackendChoice,
  sdk_es2abc_path: PathBuf,
  target_api_version: u32,
  is_debug: bool,
}

impl RolldownBytecodeCompiler {
  pub fn new(
    backend: RolldownBytecodeBackendChoice,
    sdk_es2abc_path: PathBuf,
    target_api_version: u32,
    is_debug: bool,
  ) -> Self {
    Self {
      backend,
      sdk_es2abc_path,
      target_api_version,
      is_debug,
    }
  }

  pub fn compile(&self, input: RolldownBytecodeCompileInput) -> Result<Ts2abcOutput, Ts2abcError> {
    if input.module_source_files.is_empty() {
      return Err(Ts2abcError::MissingValue(
        "hvigor module source list is empty for ABC compilation".to_string(),
      ));
    }

    match self.backend {
      RolldownBytecodeBackendChoice::Auto | RolldownBytecodeBackendChoice::Sdk => {}
    }

    let mut builder = Ts2abcPlanBuilder::new(self.target_api_version)
      .with_output(Ts2abcOutputPlan {
        abc_output: input.modules_abc_path.clone(),
        proto_output: None,
        files_info_path: Some(input.cache_dir.join("filesInfo.txt")),
        cache_file: Some(
          input
            .cache_file
            .clone()
            .unwrap_or_else(|| input.cache_dir.join("modules.cache")),
        ),
      })
      .with_annotations()
      .with_opt_level(if self.is_debug { 0 } else { 2 });

    let npm_entries_path = input.cache_dir.join("npmEntries.txt");
    builder = builder.with_npm_module_cache_entry(npm_entries_path.clone());
    if !input.byte_code_har {
      let npm_entry_manifest = NpmModuleEntryManifest::from_hvigor_entries_and_external_requests(
        input.npm_entries,
        &input.external_module_requests,
      );
      npm_entry_manifest.write_at(&npm_entries_path)?;
      builder = builder.with_npm_module_entry_list(npm_entries_path);
    }

    if self.is_debug {
      builder = builder.with_debug_info();
    }
    if let Some(path) = input.compile_context_info_path {
      builder = builder.with_compile_context_info(path);
    }

    for source in &input.module_source_files {
      let source_input = ModuleSourceInput::File {
        path: source.path.clone(),
      };
      builder = builder.with_compile_unit(CompileUnitPlan {
        source: source_input,
        record_name: RecordNamePlan::Explicit(source.record_name.clone()),
        source_file: Some(source.source_file.clone()),
        package_name: source.package_name.clone(),
        is_commonjs: source.is_commonjs,
        is_shared_module: source.is_shared_module,
        origin_source_lang: source.origin_source_lang.clone(),
        output_path: None,
        cache_file: Some(source.path.with_extension("protoBin")),
      });
    }
    if !input.abc_input_files.is_empty() {
      builder = builder.with_abc_input().with_remove_redundant_file();
      for abc_input in input.abc_input_files {
        builder =
          builder.with_abc_input_file(abc_input.path, abc_input.package_name, abc_input.cache_file);
      }
    }

    let plan = builder.build()?;
    let compiler = Ts2abcCompiler::with_sdk_es2abc_path(self.sdk_es2abc_path.clone());
    compiler.compile(&plan)
  }
}

struct NpmModuleEntryManifest {
  entries: Vec<NpmModuleEntry>,
}

impl NpmModuleEntryManifest {
  fn from_hvigor_entries_and_external_requests(
    hvigor_entries: Vec<HvigorNpmEntry>,
    requests: &[String],
  ) -> Self {
    let mut entries = hvigor_entries
      .into_iter()
      .map(|entry| NpmModuleEntry {
        entry_path: entry.entry_path,
        build_path: entry.build_path,
      })
      .collect::<Vec<_>>();
    // SDK `ModuleMode.getNativeModuleEntryInfo` inserts this fixed list
    // before graph-derived system requests. `Map.set` then keeps the first
    // insertion when the same request was already observed in module imports.
    entries.extend(HvigorNpmModuleRequest::native_module_entries());
    entries.extend(
      requests
        .iter()
        .filter_map(|request| HvigorNpmModuleRequest::new(request).entry()),
    );
    let mut seen = FxHashSet::default();
    entries.retain(|entry| seen.insert((entry.entry_path.clone(), entry.build_path.clone())));
    Self { entries }
  }

  fn write_at(&self, path: &Path) -> Result<(), Ts2abcError> {
    if let Some(parent) = path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    let mut content = String::new();
    for entry in &self.entries {
      content.push_str(&entry.entry_path);
      content.push(':');
      content.push_str(&entry.build_path);
      content.push('\n');
    }
    fs_err::write(path, content)?;
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NpmModuleEntry {
  entry_path: String,
  build_path: String,
}

struct HvigorNpmModuleRequest<'a> {
  request: &'a str,
}

impl<'a> HvigorNpmModuleRequest<'a> {
  const NATIVE_MODULES: &'static [&'static str] = &[
    "system.app",
    "ohos.app",
    "system.router",
    "system.curves",
    "ohos.curves",
    "system.matrix4",
    "ohos.matrix4",
  ];

  fn new(request: &'a str) -> Self {
    Self { request }
  }

  fn native_module_entries() -> Vec<NpmModuleEntry> {
    Self::NATIVE_MODULES
      .iter()
      .map(|module| NpmModuleEntry {
        entry_path: ["@", *module].concat(),
        build_path: ["@native.", *module].concat(),
      })
      .collect()
  }

  fn entry(&self) -> Option<NpmModuleEntry> {
    if let Some(module) = self.request.strip_prefix("@ohos:") {
      return self.system_entry("ohos", module);
    }
    if let Some(module) = self.request.strip_prefix("@ohos.") {
      return self.system_entry("ohos", module);
    }
    if let Some(module) = self.request.strip_prefix("@system.") {
      return self.system_entry("system", module);
    }
    if let Some(module) = self.request.strip_prefix("@ohos:system.") {
      return self.system_entry("system", module);
    }
    None
  }

  fn system_entry(&self, prefix: &str, module: &str) -> Option<NpmModuleEntry> {
    let full_name = [prefix, module].join(".");
    let entry_path = ["@", full_name.as_str()].concat();
    Self::NATIVE_MODULES
      .contains(&full_name.as_str())
      .then(|| NpmModuleEntry {
        entry_path,
        build_path: ["@native.", full_name.as_str()].concat(),
      })
  }
}

impl Ts2abcCompiler {
  /// Create a compiler that can execute SDK es2abc plans.
  pub fn with_sdk_es2abc_path(sdk_es2abc_path: PathBuf) -> Self {
    Self {
      sdk_es2abc_path: Some(sdk_es2abc_path),
    }
  }

  /// Compile a typed plan.
  pub fn compile(&self, plan: &Ts2abcPlan) -> Result<Ts2abcOutput, Ts2abcError> {
    plan.validate()?;
    self.compile_with_sdk(plan)
  }

  fn compile_with_sdk(&self, plan: &Ts2abcPlan) -> Result<Ts2abcOutput, Ts2abcError> {
    let es2abc_path = self
      .sdk_es2abc_path
      .as_ref()
      .ok_or_else(|| Ts2abcError::MissingValue("sdk es2abc path is required".to_string()))?;
    Self::validate_sdk_es2abc(es2abc_path)?;

    let mut options = self.plan_to_driver_options(plan)?;
    let input_file_list = if let Some(path) = &plan.output.files_info_path {
      self.write_file_info_at(path, plan)?;
      Some(path.clone())
    } else {
      None
    };
    if let Some(cache_file) = &plan.output.cache_file {
      self.write_cache_manifest_if_needed(cache_file, plan)?;
    }

    options.cache_file = plan.output.cache_file.clone();
    let backend = SdkEs2abcBackend::new(es2abc_path.clone()).with_input_file_list(input_file_list);
    backend.compile(&options)
  }

  fn plan_to_driver_options(&self, plan: &Ts2abcPlan) -> Result<Ts2abcOptions, Ts2abcError> {
    let inputs: Vec<Ts2abcInput> = plan
      .compile_units
      .iter()
      .map(|unit| {
        let path = match &unit.source {
          ModuleSourceInput::File { path } => path.clone(),
          ModuleSourceInput::AbcFile { path } => path.clone(),
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

    let compile_context_info = if let Some(ctx_path) = &plan.compile_context_info_path {
      CompileContextInfo::read_upstream_lenient(ctx_path).unwrap_or_default()
    } else {
      CompileContextInfo::default()
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
      compile_context_info,
      enable_abc_input: plan.enable_abc_input,
      dump_deps_info: plan.dump_deps_info,
      remove_redundant_file: plan.remove_redundant_file,
      npm_module_entry_list: plan.npm_module_entry_list.clone(),
    })
  }

  fn validate_sdk_es2abc(path: &Path) -> Result<(), Ts2abcError> {
    if !path.exists() {
      return Err(Ts2abcError::SdkCompilerNotFound(path.to_path_buf()));
    }
    if !path.is_file() {
      return Err(Ts2abcError::SdkCompilerNotExecutable(path.to_path_buf()));
    }
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mode = fs_err::metadata(path)?.permissions().mode();
      if mode & 0o111 == 0 {
        return Err(Ts2abcError::SdkCompilerNotExecutable(path.to_path_buf()));
      }
    }
    Ok(())
  }

  fn write_file_info_at(&self, path: &Path, plan: &Ts2abcPlan) -> Result<(), Ts2abcError> {
    if let Some(parent) = path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    let policy = FileInfoColumnPolicy::merge_columns(plan.merge_abc);
    let mut content = FileInfoManifestText::new();
    for unit in &plan.compile_units {
      let line = FileInfoLine::from_compile_unit(unit, policy)?;
      content.push_line(&line);
    }
    fs_err::write(path, content.into_string())?;
    Ok(())
  }

  fn write_cache_manifest_if_needed(
    &self,
    cache_file: &Path,
    plan: &Ts2abcPlan,
  ) -> Result<(), Ts2abcError> {
    if cache_file.file_name().and_then(|name| name.to_str()) != Some("modules.cache") {
      return Ok(());
    }
    let Some(parent) = cache_file.parent() else {
      return Ok(());
    };
    let writer = CacheFileManifestWriter::new(parent, "modules.cache");
    writer.write_with_npm_entries(&plan.compile_units, plan.npm_module_cache_entry.as_deref())?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::fs;

  use super::*;
  use crate::cmd::build::bytecode::plan::{
    CompileUnitPlan, ModuleSourceInput, RecordNamePlan, Ts2abcOutputPlan, Ts2abcPlan,
  };

  fn plan(root: &Path) -> Ts2abcPlan {
    Ts2abcPlan {
      compile_units: vec![CompileUnitPlan {
        source: ModuleSourceInput::File {
          path: root.join("entry.js"),
        },
        record_name: RecordNamePlan::Explicit("entry".to_string()),
        source_file: Some("entry.js".to_string()),
        package_name: Some("entry".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "js".to_string(),
        output_path: None,
        cache_file: None,
      }],
      output: Ts2abcOutputPlan {
        abc_output: root.join("modules.abc"),
        proto_output: None,
        files_info_path: Some(root.join("filesInfo.txt")),
        cache_file: None,
      },
      target_api_version: 12,
      target_api_sub_version: None,
      debug_info: true,
      opt_level: 0,
      optimize_try_catch_func: true,
      merge_abc: true,
      module_mode: true,
      enable_annotations: true,
      use_define_semantic: false,
      compile_context_info_path: None,
      dump_deps_info: false,
      remove_redundant_file: false,
      npm_module_entry_list: None,
      npm_module_cache_entry: None,
      enable_abc_input: false,
    }
  }

  struct FakeEs2abc;

  impl FakeEs2abc {
    fn write(root: &Path, args_log: Option<&Path>) -> PathBuf {
      let path = root.join(if cfg!(windows) {
        "es2abc.cmd"
      } else {
        "es2abc"
      });
      let script = if cfg!(windows) {
        let log_argument = args_log
          .map(|path| format!(r#">>"{}" echo %~1"#, path.display()))
          .unwrap_or_default();
        format!(
          "@echo off\r\nsetlocal\r\n:loop\r\nif \"%~1\"==\"\" goto done\r\n{log_argument}\r\nif not \"%~1\"==\"--output\" goto next\r\nshift\r\ntype nul > \"%~1\"\r\n:next\r\nshift\r\ngoto loop\r\n:done\r\n"
        )
      } else {
        let log_argument = args_log
          .map(|path| format!(r#"printf '%s\n' "$@" > "{}""#, path.display()))
          .unwrap_or_default();
        format!(
          "#!/bin/sh\n{log_argument}\nwhile [ \"$1\" != \"\" ]; do if [ \"$1\" = \"--output\" ]; then shift; touch \"$1\"; fi; shift; done\n"
        )
      };
      fs::write(&path, script).unwrap();
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
      }
      path
    }
  }

  #[test]
  fn sdk_backend_requires_path() {
    let tmp = tempfile::tempdir().unwrap();
    let plan = plan(tmp.path());
    let err = Ts2abcCompiler::default().compile(&plan).unwrap_err();
    assert!(matches!(err, Ts2abcError::MissingValue(_)));
  }

  #[test]
  fn sdk_backend_rejects_missing_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let plan = plan(tmp.path());
    let err = Ts2abcCompiler::with_sdk_es2abc_path(tmp.path().join("missing-es2abc"))
      .compile(&plan)
      .unwrap_err();
    assert!(matches!(err, Ts2abcError::SdkCompilerNotFound(_)));
  }

  #[test]
  fn writes_fixed_files_info_before_sdk_spawn() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("entry.js"), "export {};").unwrap();
    let es2abc = FakeEs2abc::write(tmp.path(), None);

    let plan = plan(tmp.path());
    let output = Ts2abcCompiler::with_sdk_es2abc_path(es2abc)
      .compile(&plan)
      .unwrap();
    assert_eq!(output.abc_path, tmp.path().join("modules.abc"));
    let files_info = fs::read_to_string(tmp.path().join("filesInfo.txt")).unwrap();
    assert!(files_info.contains("entry;esm;entry.js;entry;false;js"));
  }

  #[test]
  fn rolldown_bytecode_rejects_missing_hvigor_module_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let compiler = RolldownBytecodeCompiler::new(
      RolldownBytecodeBackendChoice::Sdk,
      tmp.path().join("es2abc"),
      12,
      true,
    );
    let input = RolldownBytecodeCompileInput {
      module_source_files: vec![],
      external_module_requests: vec![],
      modules_abc_path: tmp.path().join("modules.abc"),
      cache_dir: tmp.path().join("cache"),
      npm_entries: Vec::new(),
      cache_file: None,
      abc_input_files: Vec::new(),
      compile_context_info_path: None,
      byte_code_har: false,
    };

    let err = compiler.compile(input).unwrap_err();
    assert!(
      matches!(err, Ts2abcError::MissingValue(message) if message.contains("hvigor module source list"))
    );
  }

  #[test]
  fn rolldown_bytecode_uses_hvigor_module_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let module_source = tmp.path().join("module_sources/phone/ets/pages/Index.js");
    fs::create_dir_all(module_source.parent().unwrap()).unwrap();
    fs::write(&module_source, "export {};").unwrap();
    let args_log = tmp.path().join("es2abc.args");
    let es2abc = FakeEs2abc::write(tmp.path(), Some(&args_log));

    let compiler =
      RolldownBytecodeCompiler::new(RolldownBytecodeBackendChoice::Sdk, es2abc, 12, true);
    compiler
      .compile(RolldownBytecodeCompileInput {
        module_source_files: vec![HvigorModuleSourceFile {
          module_id: "/project/phone/src/main/ets/pages/Index.ets".to_string(),
          path: module_source,
          record_name: "&phone/src/main/ets/pages/Index&".to_string(),
          source_file: "src/main/ets/pages/Index.ets".to_string(),
          package_name: Some("com.example.hmosworld".to_string()),
          is_commonjs: false,
          is_shared_module: false,
          origin_source_lang: "ets".to_string(),
          imported_record_names: vec![],
        }],
        external_module_requests: vec![],
        modules_abc_path: tmp.path().join("modules.abc"),
        cache_dir: tmp.path().join("cache"),
        npm_entries: Vec::new(),
        cache_file: None,
        abc_input_files: Vec::new(),
        compile_context_info_path: None,
        byte_code_har: false,
      })
      .unwrap();

    let files_info = fs::read_to_string(tmp.path().join("cache/filesInfo.txt")).unwrap();
    assert!(
            files_info.contains("&phone/src/main/ets/pages/Index&;esm;src/main/ets/pages/Index.ets;com.example.hmosworld;false;ets"),
            "{files_info}"
        );
    assert!(fs::read_to_string(tmp.path().join("cache/modules.cache"))
      .unwrap()
      .contains("module_sources/phone/ets/pages/Index.js"));
    let modules_cache = fs::read_to_string(tmp.path().join("cache/modules.cache")).unwrap();
    assert!(modules_cache.contains("npmEntries.txt;"), "{modules_cache}");
    assert!(
      modules_cache.contains("npmEntries.protoBin"),
      "{modules_cache}"
    );
    let args = fs::read_to_string(args_log).unwrap();
    assert!(args.contains("--file-threads\n"), "{args}");
    assert!(args.contains("--enable-release-column\n"), "{args}");
    assert!(args.contains("--enable-callable-name\n"), "{args}");
    assert!(!args.contains("--module\n"), "{args}");
  }

  #[test]
  fn byte_code_har_omits_npm_entry_manifest_and_argument_but_keeps_cache_row() {
    let tmp = tempfile::tempdir().unwrap();
    let module_source = tmp.path().join("module_sources/har/ets/Index.js");
    fs::create_dir_all(module_source.parent().unwrap()).unwrap();
    fs::write(&module_source, "export {};").unwrap();
    let args_log = tmp.path().join("es2abc.args");
    let es2abc = FakeEs2abc::write(tmp.path(), Some(&args_log));

    let compiler =
      RolldownBytecodeCompiler::new(RolldownBytecodeBackendChoice::Sdk, es2abc, 12, true);
    compiler
      .compile(RolldownBytecodeCompileInput {
        module_source_files: vec![HvigorModuleSourceFile {
          module_id: "/project/har/src/main/ets/Index.ets".to_string(),
          path: module_source,
          record_name: "@normalized:N&&&har/Index&".to_string(),
          source_file: "har|har|1.0.0|src/main/ets/Index.ets".to_string(),
          package_name: Some("har".to_string()),
          is_commonjs: false,
          is_shared_module: false,
          origin_source_lang: "ets".to_string(),
          imported_record_names: vec![],
        }],
        external_module_requests: vec!["@ohos.app".to_string()],
        modules_abc_path: tmp.path().join("modules.abc"),
        cache_dir: tmp.path().join("cache"),
        npm_entries: Vec::new(),
        cache_file: None,
        abc_input_files: Vec::new(),
        compile_context_info_path: None,
        byte_code_har: true,
      })
      .unwrap();

    let npm_entries = tmp.path().join("cache/npmEntries.txt");
    assert!(
      !npm_entries.exists(),
      "SDK ModuleMode.genDescriptionsForMergedEs2abc skips npmEntries.txt for ByteCode HAR"
    );
    let modules_cache = fs::read_to_string(tmp.path().join("cache/modules.cache")).unwrap();
    assert!(modules_cache.contains("npmEntries.txt;"), "{modules_cache}");
    assert!(
      modules_cache.contains("npmEntries.protoBin"),
      "{modules_cache}"
    );
    let args = fs::read_to_string(args_log).unwrap();
    assert!(!args.contains("--npm-module-entry-list\n"), "{args}");
  }

  #[test]
  fn npm_entries_match_hvigor_native_module_mapping() {
    let tmp = tempfile::tempdir().unwrap();
    let external_requests = vec![
      "@ohos.matrix4".to_string(),
      "@ohos.curves".to_string(),
      "@system.app".to_string(),
    ];
    let manifest = NpmModuleEntryManifest::from_hvigor_entries_and_external_requests(
      Vec::new(),
      &external_requests,
    );
    let path = tmp.path().join("npmEntries.txt");
    manifest.write_at(&path).unwrap();

    let content = fs::read_to_string(path).unwrap();
    assert_eq!(
            content,
            "@system.app:@native.system.app\n@ohos.app:@native.ohos.app\n@system.router:@native.system.router\n@system.curves:@native.system.curves\n@ohos.curves:@native.ohos.curves\n@system.matrix4:@native.system.matrix4\n@ohos.matrix4:@native.ohos.matrix4\n"
        );
  }
}
