use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;

use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{
    ArkdownConfig, BuildMode, HapManifest, HvigorModuleSourceFile, ModuleConfig,
    StaticArktsSdkLayout,
};
use arkdown_path::{lexical_normalize, ArtifactPathSerializer};
use arkdown_profile::LoaderJson;
use arkdown_session::dependency_graph::snapshot::{DependencyNodeSnapshot, DependencyNodeType};
use arkdown_session::{ModuleCompileContext, SourceStore};

use crate::layout::ModuleBuildLayout;

pub struct StaticArktsCompileInput<'a> {
    pub config: &'a ArkdownConfig,
    pub module: &'a ModuleConfig,
    pub manifest: &'a HapManifest,
    pub loader_json: &'a LoaderJson,
    pub module_context: &'a ModuleCompileContext,
    pub module_layout: &'a ModuleBuildLayout<'a>,
    pub loader_json_path: &'a Path,
    pub module_source_files: &'a [HvigorModuleSourceFile],
    pub node_executable: PathBuf,
    pub output_abc: &'a Path,
    pub preview_mode: bool,
    pub check_entry: bool,
    pub is_crossplatform: bool,
    pub ui_transform_optimization: bool,
    pub bundle_type: &'a str,
    pub obfuscation: Option<&'a StaticArktsObfuscationPlan>,
}

/// Static-driver obfuscation handoff observed in
/// `obfuscation_config.ts::genObfuscationConfig` and
/// `obfuscation_bytecode_executor_invoke.ts::executeObfuscator`.
#[derive(Debug, Clone)]
pub struct StaticArktsObfuscationPlan {
    driver_options: StaticArktsDriverObfuscationOptions,
    cache_dir: PathBuf,
    ark_guard: PathBuf,
    panda_lib: PathBuf,
    debugging: bool,
}

impl StaticArktsObfuscationPlan {
    pub fn new(
        sdk: &StaticArktsSdkLayout,
        rule_files: &[PathBuf],
        consumer_rule_files: &[PathBuf],
        dependency_rule_files: &[PathBuf],
        cache_dir: PathBuf,
    ) -> Result<Option<Self>> {
        let selection = StaticArktsObfuscationRuleSelection::from_rule_files(rule_files)?;
        if !selection.enable_bytecode || selection.disable_obfuscation {
            return Ok(None);
        }
        let paths = |values: &[PathBuf]| {
            values
                .iter()
                .map(|path| ArtifactPathSerializer::path_to_slash_string(path))
                .collect::<std::result::Result<Vec<_>, _>>()
        };
        Ok(Some(Self {
            driver_options: StaticArktsDriverObfuscationOptions {
                self_config: StaticArktsDriverSelfObfuscation {
                    rule_options: StaticArktsDriverRuleOptions {
                        enable: true,
                        rules: paths(rule_files)?,
                    },
                    consumer_rules: paths(consumer_rule_files)?,
                    lib_dir: String::new(),
                },
                sdk_apis: paths(&[sdk.kits.clone(), sdk.api.clone(), sdk.arkts.clone()])?,
                obfuscation_cache_dir: ArtifactPathSerializer::path_to_slash_string(&cache_dir)?,
                export_rule_path: String::new(),
                dependencies: StaticArktsDriverObfuscationDependencies {
                    libraries: Vec::new(),
                    hsp_libraries: Vec::new(),
                    // `ObConfigResolver::getDependencyConfigs` merges the HAR
                    // and HSP file lists with identical semantics. Arkdown's
                    // resolved dependency graph intentionally exposes the
                    // flattened rule inventory, so pass it once here.
                    hars: paths(dependency_rule_files)?,
                    hsps: Vec::new(),
                },
            },
            cache_dir,
            ark_guard: sdk.ark_guard.clone(),
            panda_lib: sdk.panda_sdk.join("lib"),
            debugging: selection.debugging,
        }))
    }

    fn driver_options(&self) -> &StaticArktsDriverObfuscationOptions {
        &self.driver_options
    }

    pub fn execute(&self, modules_abc: &Path) -> Result<()> {
        if !self.ark_guard.is_file() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_ARK_GUARD_MISSING: static bytecode obfuscation requires {}",
                self.ark_guard.display()
            );
        }
        if !modules_abc.is_file() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_OBFUSCATION_INPUT_MISSING: {}",
                modules_abc.display()
            );
        }
        let config_path = self.cache_dir.join("config.json");
        let generated: StaticArktsGeneratedObfuscationConfig =
            serde_json::from_slice(&fs_err::read(&config_path).with_context(|| {
                format!(
                    "ARKDOWN_ARKTS12_OBFUSCATION_CONFIG_MISSING: {}",
                    config_path.display()
                )
            })?)?;
        let configured_abc = PathBuf::from(&generated.abc_path);
        if configured_abc != modules_abc {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_OBFUSCATION_ABC_MISMATCH: driver configured {}, build produced {}",
                configured_abc.display(),
                modules_abc.display()
            );
        }

        let mut command = Command::new(&self.ark_guard);
        if self.debugging {
            command
                .arg("--debug")
                .arg("--debug-file")
                .arg(self.cache_dir.join("debug.txt"));
        }
        command.arg(&config_path);
        self.prepend_native_library_path(&mut command);
        let output = command
            .output()
            .with_context(|| format!("launch ArkTS 1.2 ark_guard {}", self.ark_guard.display()))?;
        if !output.status.success() || !output.stderr.is_empty() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_OBFUSCATION_FAILED: ark_guard exited with {}; stdout: {}; stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let obfuscated_abc = PathBuf::from(&generated.obf_abc_path);
        if !obfuscated_abc.is_file() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_OBFUSCATION_OUTPUT_MISSING: {}",
                obfuscated_abc.display()
            );
        }
        let origin_dir = obfuscated_abc
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| anyhow::anyhow!("invalid ark_guard output path"))?
            .join("origin");
        fs_err::create_dir_all(&origin_dir)?;
        let origin_abc = origin_dir.join(
            obfuscated_abc
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("ark_guard output has no file name"))?,
        );
        fs_err::copy(modules_abc, &origin_abc)?;
        fs_err::copy(&obfuscated_abc, modules_abc)?;
        Ok(())
    }

    fn prepend_native_library_path(&self, command: &mut Command) {
        let variable = if cfg!(target_os = "macos") {
            "DYLD_LIBRARY_PATH"
        } else if cfg!(windows) {
            "PATH"
        } else {
            "LD_LIBRARY_PATH"
        };
        let mut paths = vec![self.panda_lib.clone()];
        if let Some(existing) = std::env::var_os(variable) {
            paths.extend(std::env::split_paths(&existing));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            command.env(variable, joined);
        }
    }
}

/// Execution switches owned by the module's rule files. The static SDK
/// `ObConfigResolver::getMergedConfigs` only merges keep options from
/// dependencies, so dependency consumer rules must never select or disable
/// bytecode mode for the current module.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct StaticArktsObfuscationRuleSelection {
    enable_bytecode: bool,
    disable_obfuscation: bool,
    debugging: bool,
}

impl StaticArktsObfuscationRuleSelection {
    fn from_rule_files(rule_files: &[PathBuf]) -> Result<Self> {
        let mut selection = Self::default();
        for path in rule_files {
            let source = fs_err::read_to_string(path).with_context(|| {
                format!("ARKDOWN_ARKTS12_OBFUSCATION_RULE_READ: {}", path.display())
            })?;
            selection.merge_source(&source);
        }
        Ok(selection)
    }

    fn merge_source(&mut self, source: &str) {
        let mut uncommented = String::with_capacity(source.len());
        let mut in_comment = false;
        for character in source.chars() {
            if in_comment {
                if character == '\n' {
                    in_comment = false;
                    uncommented.push(character);
                }
            } else if character == '#' {
                in_comment = true;
            } else {
                uncommented.push(character);
            }
        }

        for directive in uncommented.lines().filter_map(|line| {
            let line = line.trim();
            line.starts_with('-')
                .then(|| line.split_whitespace().next())
                .flatten()
        }) {
            match directive {
                "-enable-bytecode-obfuscation" => self.enable_bytecode = true,
                "-disable-obfuscation" => self.disable_obfuscation = true,
                "-enable-bytecode-obfuscation-debugging" => self.debugging = true,
                _ => {}
            }
        }
    }
}

/// ArkTS 1.2 SDK frontend adapter.
///
/// OXC and Rolldown own parsing and graph construction. The SDK driver is
/// the source-defined host for the parsed/checked ArkUI plugins and the native
/// es2panda state machine.
pub struct StaticArktsCompiler<'a> {
    input: StaticArktsCompileInput<'a>,
    sdk: StaticArktsSdkLayout,
}

/// Generated handoff that lets the dynamic Rolldown/es2abc half of a mixed
/// ArkTS 1.2 build consume SDK-produced bridge sources.
pub struct StaticArktsInteropOutput {
    pub bridge_root: PathBuf,
    pub source_redirects: FxHashMap<PathBuf, PathBuf>,
    pub requires_dynamic_abc: bool,
}

impl StaticArktsInteropOutput {
    pub fn redirect_entries(&self, module_root: &Path, entries: &mut [String]) -> Result<()> {
        for entry in entries {
            let (specifier, query) = entry
                .split_once('?')
                .map_or((entry.as_str(), None), |(path, query)| (path, Some(query)));
            let path = Path::new(specifier);
            let absolute = if path.has_root() {
                lexical_normalize(path)
            } else {
                lexical_normalize(
                    &module_root.join(specifier.strip_prefix("./").unwrap_or(specifier)),
                )
            };
            let Some(bridge) = self.source_redirects.get(&absolute) else {
                continue;
            };
            let mut redirected = ArtifactPathSerializer::path_to_slash_string(bridge)?;
            if let Some(query) = query {
                redirected.push('?');
                redirected.push_str(query);
            }
            *entry = redirected;
        }
        Ok(())
    }
}

impl<'a> StaticArktsCompiler<'a> {
    pub fn new(input: StaticArktsCompileInput<'a>) -> Result<Self> {
        let sdk = input
            .config
            .sdk_config
            .static_arkts_layout()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_SDK_LAYOUT: cannot derive ets/static from {}",
                    input.config.sdk_config.ets_loader_path.display()
                )
            })?;
        let compiler = Self { input, sdk };
        compiler.validate_sdk()?;
        Ok(compiler)
    }

    pub fn compile(&self) -> Result<Option<PathBuf>> {
        let build_config = StaticArktsDriverConfigBuilder::new(&self.input, &self.sdk).build()?;
        let config_path = PathBuf::from(&self.input.loader_json.cache_path)
            .join("arkts12")
            .join("build_config.json");
        if let Some(parent) = config_path.parent() {
            fs_err::create_dir_all(parent).with_context(|| {
                format!("create ArkTS 1.2 config directory {}", parent.display())
            })?;
        }
        fs_err::write(&config_path, serde_json::to_vec_pretty(&build_config)?)
            .with_context(|| format!("write ArkTS 1.2 config {}", config_path.display()))?;
        let output_abc = self.output_abc()?;
        if !build_config.has_static_output_inputs() {
            // `BaseMode::mergeAbcFiles` returns without creating
            // `modules_static.abc` when there are neither static entries nor
            // linkable ArkTS 1.2 ByteCode HAR inputs. This is the normal shape
            // of an ArkTS 1.2 product whose module contains only dynamic code.
            if output_abc.is_file() {
                fs_err::remove_file(&output_abc).with_context(|| {
                    format!("remove stale ArkTS 1.2 output {}", output_abc.display())
                })?;
            }
            return Ok(None);
        }
        let output_dir = output_abc
            .parent()
            .expect("the static ABC path is always below loaderOutPath");
        fs_err::create_dir_all(output_dir)
            .with_context(|| format!("create ArkTS 1.2 output {}", output_dir.display()))?;

        let output = self.command(&config_path).output().with_context(|| {
            format!(
                "launch ArkTS 1.2 SDK driver {}",
                self.sdk.driver_entry.display()
            )
        })?;
        if !output.status.success() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_COMPILE_FAILED: driver {} exited with {}; stdout: {}; stderr: {}",
                self.sdk.driver_entry.display(),
                output.status,
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        if !output_abc.is_file() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_OUTPUT_MISSING: static compiler completed without writing {}; stdout: {}; stderr: {}",
                output_abc.display(),
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(Some(output_abc))
    }

    pub fn requires_dynamic_abc(&self) -> Result<bool> {
        let builder = StaticArktsDriverConfigBuilder::new(&self.input, &self.sdk);
        Ok(!builder.main_module()?.dynamic_files.is_empty()
            || builder
                .build_declgen()?
                .dependency_module_list
                .iter()
                .any(|module| !module.byte_code_har && !module.dynamic_files.is_empty()))
    }

    /// Run the current SDK's two declaration producers before Rolldown scans a
    /// mixed 1.1/1.2 graph. This follows
    /// `interop_manager.ts` + `run_declgen_standalone.ts`; generated files are
    /// build intermediates and never mutate project sources.
    pub fn prepare_interop(&self) -> Result<StaticArktsInteropOutput> {
        let builder = StaticArktsDriverConfigBuilder::new(&self.input, &self.sdk);
        let build_config = builder.build_declgen()?;
        let mut modules = build_config
            .dependency_module_list
            .iter()
            .filter(|module| !module.byte_code_har)
            .cloned()
            .collect::<Vec<_>>();
        let main_module = builder.main_module()?;
        if !modules
            .iter()
            .any(|module| module.package_name == main_module.package_name)
        {
            modules.insert(0, main_module);
        }
        let requires_dynamic_abc = modules
            .iter()
            .any(|module| !module.dynamic_files.is_empty());
        let bridge_root = self.input.module_layout.arkts_declgen_bridge_dir();
        if !requires_dynamic_abc {
            let request_path = self.dynamic_declgen_request_path();
            if request_path.is_file() {
                fs_err::remove_file(request_path)?;
            }
            return Ok(StaticArktsInteropOutput {
                bridge_root,
                source_redirects: FxHashMap::default(),
                requires_dynamic_abc: false,
            });
        }

        let interop_plugin =
            Self::existing_javascript_entry(&self.sdk.interop_plugin).ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_INTEROP_PLUGIN_MISSING: {}[.js]",
                    self.sdk.interop_plugin.display()
                )
            })?;
        let dynamic_loader = self.input.config.sdk_config.ets_loader_path.join("main.js");
        if !dynamic_loader.is_file() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_DYNAMIC_DECLGEN_MISSING: {}",
                dynamic_loader.display()
            );
        }

        let mut declgen_config = build_config;
        declgen_config.plugins = IndexMap::from([(
            "interop_plugin".to_string(),
            ArtifactPathSerializer::path_to_slash_string(&interop_plugin)?,
        )]);
        let config_path = PathBuf::from(&self.input.loader_json.cache_path)
            .join("arkts12")
            .join("declgen_build_config.json");
        self.write_driver_config(&config_path, &declgen_config)?;
        if !declgen_config.compile_files.is_empty() {
            let output = self.command(&config_path).output().with_context(|| {
                format!(
                    "launch ArkTS 1.2 declaration driver {}",
                    self.sdk.driver_entry.display()
                )
            })?;
            if !output.status.success() {
                anyhow::bail!(
                    "ARKDOWN_ARKTS12_DECLGEN_V1_FAILED: driver exited with {}; stdout: {}; stderr: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout).trim(),
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
        }

        self.run_dynamic_declgen(&dynamic_loader, &modules)?;
        self.write_decl_file_indexes(&modules)?;
        let source_redirects = self.verify_interop_outputs(&modules)?;
        Ok(StaticArktsInteropOutput {
            bridge_root,
            source_redirects,
            requires_dynamic_abc,
        })
    }

    fn existing_javascript_entry(path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
        let javascript = path.with_extension("js");
        javascript.is_file().then_some(javascript)
    }

    fn write_driver_config(&self, path: &Path, config: &StaticArktsDriverConfig) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(path, serde_json::to_vec_pretty(config)?)
            .with_context(|| format!("write ArkTS 1.2 config {}", path.display()))
    }

    fn run_dynamic_declgen(
        &self,
        dynamic_loader: &Path,
        modules: &[StaticArktsDependentModule],
    ) -> Result<()> {
        let request_path = self.dynamic_declgen_request_path();
        if let Some(parent) = request_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let request = StaticArktsDynamicDeclgenRequest {
            modules,
            cache_path: &self.input.loader_json.cache_path,
            bundle_name: &self.input.config.product.bundle_name,
            main_module_name: &self.input.loader_json.module_name,
            project_root_path: &self.input.config.project_root,
        };
        fs_err::write(&request_path, serde_json::to_vec_pretty(&request)?)?;
        let adapter = r#"const fs=require('fs');const loader=require(process.argv[1]);const r=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));const modules=new Map(r.modules.map(m=>[m.packageName,m]));Promise.resolve(loader.runDeclgenTs2Ets({dependentModuleMap:modules,projectConfig:{cachePath:r.cachePath,bundleName:r.bundleName,mainModuleName:r.mainModuleName,projectRootPath:r.projectRootPath,sdkAliasMap:new Map()}})).catch(e=>{console.error(e&&e.stack||e);process.exitCode=1;});"#;
        let output = Command::new(&self.input.node_executable)
            .arg("-e")
            .arg(adapter)
            .arg(dynamic_loader)
            .arg(&request_path)
            .output()
            .with_context(|| {
                format!(
                    "launch ArkTS 1.1 declaration generator {}",
                    dynamic_loader.display()
                )
            })?;
        if !output.status.success() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_DECLGEN_V2_FAILED: loader exited with {}; stdout: {}; stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    fn dynamic_declgen_request_path(&self) -> PathBuf {
        self.input
            .module_layout
            .arkts_interop_dir()
            .join("declgen-v2-request.json")
    }

    fn write_decl_file_indexes(&self, modules: &[StaticArktsDependentModule]) -> Result<()> {
        for module in modules
            .iter()
            .filter(|module| !module.dynamic_files.is_empty())
        {
            let index_path = module.decl_files_path.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_DECL_FILE_INFO_PATH_MISSING: {}",
                    module.package_name
                )
            })?;
            let module_root = Path::new(&module.module_path);
            let declgen_v2 = Path::new(module.declgen_v2_out_path.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_DECLGEN_V2_PATH_MISSING: {}",
                    module.package_name
                )
            })?);
            let mut files = BTreeMap::new();
            for source in &module.dynamic_files {
                let source = Path::new(source);
                let relative = source.strip_prefix(module_root).map_err(|_| {
                    anyhow::anyhow!(
                        "ARKDOWN_ARKTS12_INTEROP_SOURCE_ESCAPE: {} is outside {}",
                        source.display(),
                        module_root.display()
                    )
                })?;
                let key = StaticArktsInteropPath::without_source_extension(relative)?;
                let decl_path = declgen_v2.join(format!("{key}.d.ets"));
                files.insert(
                    key.clone(),
                    StaticArktsDeclFileRecord {
                        decl_path: ArtifactPathSerializer::path_to_slash_string(&decl_path)?,
                        file_path: ArtifactPathSerializer::path_to_slash_string(source)?,
                        ohm_url: format!("@normalized:N&&&{}/{}&", module.package_name, key),
                    },
                );
            }
            let index = StaticArktsDeclFileIndex {
                package_name: &module.package_name,
                files,
            };
            let index_path = Path::new(index_path);
            if let Some(parent) = index_path.parent() {
                fs_err::create_dir_all(parent)?;
            }
            fs_err::write(index_path, serde_json::to_vec_pretty(&index)?)?;
        }
        Ok(())
    }

    fn verify_interop_outputs(
        &self,
        modules: &[StaticArktsDependentModule],
    ) -> Result<FxHashMap<PathBuf, PathBuf>> {
        let mut redirects = FxHashMap::default();
        for module in modules {
            let module_root = Path::new(&module.module_path);
            for source in &module.static_files {
                let source = Path::new(source);
                let relative = source.strip_prefix(module_root).map_err(|_| {
                    anyhow::anyhow!(
                        "ARKDOWN_ARKTS12_INTEROP_SOURCE_ESCAPE: {} is outside {}",
                        source.display(),
                        module_root.display()
                    )
                })?;
                let key = StaticArktsInteropPath::without_source_extension(relative)?;
                let declaration =
                    Path::new(module.declgen_v1_out_path.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("missing declgen V1 path for {}", module.package_name)
                    })?)
                    .join(&module.package_name)
                    .join(format!("{key}.d.ets"));
                let bridge =
                    Path::new(module.declgen_bridge_code_path.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("missing bridge path for {}", module.package_name)
                    })?)
                    .join(&module.package_name)
                    .join(format!("{key}.ts"));
                for (kind, output) in [("V1 declaration", &declaration), ("bridge", &bridge)] {
                    if !output.is_file() {
                        anyhow::bail!(
                            "ARKDOWN_ARKTS12_INTEROP_OUTPUT_MISSING: {kind} for {} was not generated at {}",
                            source.display(),
                            output.display()
                        );
                    }
                }
                redirects.insert(lexical_normalize(source), lexical_normalize(&bridge));
            }
            for source in &module.dynamic_files {
                let source = Path::new(source);
                let relative = source.strip_prefix(module_root)?;
                let key = StaticArktsInteropPath::without_source_extension(relative)?;
                let declaration =
                    Path::new(module.declgen_v2_out_path.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("missing declgen V2 path for {}", module.package_name)
                    })?)
                    .join(format!("{key}.d.ets"));
                if !declaration.is_file() {
                    anyhow::bail!(
                        "ARKDOWN_ARKTS12_INTEROP_OUTPUT_MISSING: V2 declaration for {} was not generated at {}",
                        source.display(),
                        declaration.display()
                    );
                }
            }
        }
        Ok(redirects)
    }

    /// `driver/build_system/src/pre_define.ts::MERGED_ABC_FILE` and
    /// `BaseMode::mergedAbcFile` fix this name below `loaderOutPath`.
    fn output_abc(&self) -> Result<PathBuf> {
        let output_dir = self
            .input
            .output_abc
            .parent()
            .ok_or_else(|| anyhow::anyhow!("ArkTS 1.2 ABC output has no parent directory"))?;
        Ok(output_dir.join("modules_static.abc"))
    }

    fn command(&self, config_path: &Path) -> Command {
        let mut command = Command::new(&self.input.node_executable);
        command.arg(&self.sdk.driver_entry).arg(config_path);
        self.prepend_native_library_path(&mut command);
        command
    }

    fn prepend_native_library_path(&self, command: &mut Command) {
        let variable = if cfg!(target_os = "macos") {
            "DYLD_LIBRARY_PATH"
        } else if cfg!(windows) {
            "PATH"
        } else {
            "LD_LIBRARY_PATH"
        };
        let mut paths = vec![self.sdk.panda_sdk.join("lib")];
        if let Some(existing) = std::env::var_os(variable) {
            paths.extend(std::env::split_paths(&existing));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            command.env(variable, joined);
        }
    }

    fn validate_sdk(&self) -> Result<()> {
        for (name, path) in [
            ("driver", &self.sdk.driver_entry),
            ("libarkts", &self.sdk.libarkts_entry),
            ("ark_link", &self.sdk.abc_linker),
            ("dependency_analyzer", &self.sdk.dependency_analyzer),
            ("etsstdlib.abc", &self.sdk.etsstdlib_abc),
        ] {
            if !path.is_file() {
                anyhow::bail!(
                    "ARKDOWN_ARKTS12_SDK_TOOL_MISSING: {name} is not available at {}",
                    path.display()
                );
            }
        }
        if !self
            .sdk
            .es2panda_addon_candidates
            .iter()
            .any(|path| path.is_file())
        {
            let candidates = self
                .sdk
                .es2panda_addon_candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!(
                "ARKDOWN_ARKTS12_SDK_ADDON_MISSING: libarkts cannot load es2panda.node; checked {candidates}"
            );
        }
        for (name, path) in [
            ("stdlib", &self.sdk.stdlib),
            ("api", &self.sdk.api),
            ("kits", &self.sdk.kits),
            ("arkts", &self.sdk.arkts),
        ] {
            if !path.is_dir() {
                anyhow::bail!(
          "ARKDOWN_ARKTS12_SDK_DECLARATIONS_MISSING: {name} declarations are not available at {}",
          path.display()
        );
            }
        }
        for (name, path) in [
            ("ui_syntax_plugin", &self.sdk.ui_syntax_plugin),
            ("ui_plugin", &self.sdk.ui_plugin),
            ("memo_plugin", &self.sdk.memo_plugin),
        ] {
            if !path.is_file() && !path.with_extension("js").is_file() {
                anyhow::bail!(
                    "ARKDOWN_ARKTS12_PLUGIN_MISSING: {name} is not available at {}[.js]",
                    path.display()
                );
            }
        }
        Ok(())
    }
}

struct StaticArktsDriverConfigBuilder<'a> {
    input: &'a StaticArktsCompileInput<'a>,
    sdk: &'a StaticArktsSdkLayout,
}

impl<'a> StaticArktsDriverConfigBuilder<'a> {
    fn new(input: &'a StaticArktsCompileInput<'a>, sdk: &'a StaticArktsSdkLayout) -> Self {
        Self { input, sdk }
    }

    fn build(&self) -> Result<StaticArktsDriverConfig> {
        self.build_for(false)
    }

    fn build_declgen(&self) -> Result<StaticArktsDriverConfig> {
        self.build_for(true)
    }

    fn build_for(&self, enable_declgen_ets2_ts: bool) -> Result<StaticArktsDriverConfig> {
        let identity = self.main_module_identity();
        StaticArktsByteCodeHarEntryContract::validate(
            &self.input.module.module_type,
            self.input.loader_json.byte_code_har.unwrap_or(false),
            self.input
                .module_context
                .module
                .package
                .as_ref()
                .and_then(|package| package.package_json.main.as_deref()),
            identity.package_name(),
        )?;
        let mut dependent_module_list = self.dependent_modules()?;
        let main_module = self.main_module()?;
        // `BaseMode::getMainModuleInfo` takes the current module's language,
        // declaration paths, and dependency aliases from its package record.
        // Omitting a hybrid main module makes the driver fall back to 1.2 and
        // consequently treats every main-module `.ets` file as static.
        dependent_module_list.insert(0, main_module);
        let compile_files = self.compile_files(&dependent_module_list)?;
        let module_files = self.module_files(&dependent_module_list)?;
        let source_roots = if self.input.module_context.module.source_roots.is_empty() {
            vec![self.path(&self.input.module_context.module.src_main)?]
        } else {
            self.input
                .module_context
                .module
                .source_roots
                .iter()
                .map(|path| self.path(path))
                .collect::<Result<Vec<_>>>()?
        };
        // `bindings/src/lsp/generateBuildConfig.ts::DEFAULT_PLUGIN_LIST`
        // defines the normal-build set/order, and
        // `PluginDriver::initPlugins` preserves object insertion order.
        // Interop is owned by the separate declgen configuration.
        let plugins = IndexMap::from([
            (
                "ui_syntax_plugin".to_string(),
                self.path(&self.sdk.ui_syntax_plugin)?,
            ),
            ("ui_plugin".to_string(), self.path(&self.sdk.ui_plugin)?),
            ("memo_plugin".to_string(), self.path(&self.sdk.memo_plugin)?),
        ]);
        let external_api_paths = [&self.sdk.api, &self.sdk.kits, &self.sdk.arkts]
            .into_iter()
            .map(|path| self.path(path))
            .collect::<Result<Vec<_>>>()?;
        let module_root = self.path(&self.input.module.src_path)?;
        let project_path = self.input.module_layout.source().ets();
        let output_dir = self
            .input
            .output_abc
            .parent()
            .ok_or_else(|| anyhow::anyhow!("ArkTS 1.2 ABC output has no parent directory"))?;
        let dependent_module_list_copy = dependent_module_list.clone();
        let output_contract = StaticArktsModuleOutputContract::new(
            &self.input.module.module_type,
            self.input.module_layout,
            &self.input.module.name,
        );
        let interop_api_paths = self
            .sdk
            .dynamic_interop
            .is_dir()
            .then(|| self.path(&self.sdk.dynamic_interop))
            .transpose()?
            .into_iter()
            .collect();

        Ok(StaticArktsDriverConfig {
            plugins,
            paths: BTreeMap::new(),
            entry_file: self.main_entry_file()?,
            compile_files,
            module_files,
            build_mode: StaticArktsBuildMode::new(&self.input.config.build_mode).to_string(),
            project_root_path: self.path(&self.input.config.project_root)?,
            module_root_path: module_root,
            ace_profile_path: self.path(&self.input.module_layout.res_profile_dir())?,
            cache_path: self.path(Path::new(&self.input.loader_json.cache_path))?,
            loader_out_path: self.path(output_dir)?,
            compile_sdk_version: self.input.config.sdk_config.compile_api_version,
            compatible_sdk_version: parse_api_version(
                &self.input.config.product.compatible_sdk_version,
            )
            .unwrap_or(self.input.config.sdk_config.compile_api_version),
            ace_module_json_path: self.path(&self.input.module_layout.res_ark_module_json())?,
            bundle_name: self.input.config.product.bundle_name.clone(),
            use_normalized_ohm_url: self.input.loader_json.use_normalized_ohm_url,
            build_type: "build".to_string(),
            package_name: identity.package_name().to_string(),
            build_sdk_path: self.path(&self.sdk.root)?,
            source_roots,
            module_type: self.input.module.module_type.to_string(),
            module_name: identity.module_name().to_string(),
            dependent_module_list,
            dependency_module_list: dependent_module_list_copy,
            has_main_module: self
                .input
                .config
                .modules
                .iter()
                .any(|module| matches!(module.module_type, arkdown_core::OhosModuleType::Entry)),
            build_loader_json: self.path(self.input.loader_json_path)?,
            integrated_hsp: self
                .input
                .config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|options| options.integrated_hsp)
                .unwrap_or(false),
            allow_empty_bundle_name: false,
            enable_declgen_ets2_ts,
            declgen_v1_out_path: Some(self.path(&self.input.module_layout.arkts_declgen_v1_dir())?),
            declgen_v2_out_path: if enable_declgen_ets2_ts {
                self.path(
                    &self
                        .input
                        .module_layout
                        .arkts_declgen_v2_dir()
                        .join(identity.package_name()),
                )?
            } else {
                output_contract
                    .declarations_root
                    .as_deref()
                    .map(|path| self.path(path))
                    .transpose()?
                    .unwrap_or_default()
            },
            declgen_bridge_code_path: Some(
                self.path(&self.input.module_layout.arkts_declgen_bridge_dir())?,
            ),
            external_api_paths,
            interop_api_paths,
            device_types: self.input.manifest.device_types.clone(),
            byte_code_har: self.input.loader_json.byte_code_har.unwrap_or(false),
            ui_transform_optimization: self.input.ui_transform_optimization,
            runtime_os: self.input.config.product.runtime_os.clone(),
            is_preview: self.input.preview_mode,
            check_entry: self.input.check_entry,
            code_root_path: self.path(&project_path)?,
            is_crossplatform: self.input.is_crossplatform,
            bundle_type: self.input.bundle_type.to_string(),
            level: StaticArktsLogLevel::default(),
            is_build_config_modified: false,
            panda_sdk_path: self.path(&self.sdk.panda_sdk)?,
            abc_linker_path: self.path(&self.sdk.abc_linker)?,
            dependency_analyzer_path: self.path(&self.sdk.dependency_analyzer)?,
            api_path: self.path(&self.sdk.api)?,
            kits_path: self.path(&self.sdk.kits)?,
            dep_analyzer_path: self.path(&self.sdk.dependency_analyzer)?,
            sdk_alias_map: BTreeMap::new(),
            es2panda_mode: output_contract.es2panda_mode.to_string(),
            app_resource: self.path(&self.input.module_layout.resource_table_txt())?,
            raw_file_resource: self
                .path(&self.input.module_layout.res_resources_dir().join("rawfile"))?,
            project_path: self.path(&project_path)?,
            hsp_resources_map: !self.input.loader_json.hsp_resources_map.is_empty(),
            compile_har: matches!(
                self.input.module.module_type,
                arkdown_core::OhosModuleType::Har
            ),
            reset_bundle_name: self
                .input
                .config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|options| options.integrated_hsp)
                .unwrap_or(false),
            ignore_error: false,
            ark_guard_path: self
                .input
                .obfuscation
                .map(|_| self.path(&self.sdk.ark_guard))
                .transpose()?,
            obfuscation_options: self
                .input
                .obfuscation
                .map(|plan| plan.driver_options().clone()),
        })
    }

    fn compile_files(
        &self,
        dependent_modules: &[StaticArktsDependentModule],
    ) -> Result<Vec<String>> {
        // `FileManager::getLanguageVersionByFilePath` makes membership in
        // `compileFiles` authoritative: every listed file is ArkTS 1.2. The
        // complete source inventory therefore belongs in module metadata,
        // while this list contains only the source-classified static subset.
        StaticArktsCompileEntryPlan::new(dependent_modules)
            .resolve()?
            .iter()
            .map(|path| self.path(path))
            .collect()
    }

    fn dependent_modules(&self) -> Result<Vec<StaticArktsDependentModule>> {
        let mut modules = Vec::new();
        let language_detector = StaticArktsLanguageDetector::new(
            self.input.module_context.session.source_store.as_ref(),
        );
        for node in self.input.module_context.deps.nodes.values() {
            if !matches!(
                node.node_type,
                DependencyNodeType::SourceHar
                    | DependencyNodeType::BytecodeHar
                    | DependencyNodeType::IntegratedHsp
                    | DependencyNodeType::SharedHsp
                    | DependencyNodeType::OhpmSource
            ) {
                continue;
            }
            let root = node.package_root.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_DEPENDENCY_ROOT_MISSING: {}",
                    node.package_name
                )
            })?;
            let package = node.package.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_DEPENDENCY_METADATA_MISSING: {}",
                    node.package_name
                )
            })?;
            let entry_file = package
                .package_json
                .main
                .as_deref()
                .map(PathBuf::from)
                .or_else(|| node.compile_inputs.dynamic_import_entry_file.clone())
                .or_else(|| node.compile_inputs.other_compile_entry_file.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "ARKDOWN_ARKTS12_DEPENDENCY_ENTRY_MISSING: package '{}' has no source-defined entry",
                        node.package_name
                    )
                })?;
            let absolute_entry_file = if entry_file.has_root() {
                lexical_normalize(&entry_file)
            } else {
                lexical_normalize(&root.join(&entry_file))
            };
            let source_root = absolute_entry_file.parent().unwrap_or(root);
            let bytecode = self
                .input
                .loader_json
                .byte_code_har_info
                .get(&node.package_name)
                .or_else(|| self.input.loader_json.byte_code_har_info.get(&node.alias));
            let language = language_detector.detect(node, bytecode.map(|entry| &entry.abc_path))?;
            let (static_files, dynamic_files) =
                self.dependency_source_languages(node, &language)?;
            let (dependencies, original_package_name_map) = self.dependency_edges(root);
            let source_interop = !matches!(node.node_type, DependencyNodeType::BytecodeHar);
            modules.push(StaticArktsDependentModule {
                package_name: node.package_name.clone(),
                module_name: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_name.clone())
                    .unwrap_or_else(|| node.package_name.clone()),
                module_type: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_type.as_ref())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| match node.node_type {
                        DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp => {
                            "shared".to_string()
                        }
                        _ => "har".to_string(),
                    }),
                module_path: self.path(root)?,
                source_roots: vec![self.path(source_root)?],
                entry_file: if entry_file.has_root() {
                    self.path(&entry_file)?
                } else {
                    ArtifactPathSerializer::rel_to_slash(&entry_file)
                },
                language: language.clone(),
                // The current driver accepts this only from the separate
                // dynamic declgen output contract. Package source inventory
                // does not own that artifact, so it must not be guessed by
                // scanning for a matching filename.
                decl_files_path: (source_interop && !dynamic_files.is_empty())
                    .then(|| {
                        self.path(
                            &self
                                .input
                                .module_layout
                                .arkts_decl_file_info(&node.package_name),
                        )
                    })
                    .transpose()?,
                declgen_v1_out_path: source_interop
                    .then(|| self.path(&self.input.module_layout.arkts_declgen_v1_dir()))
                    .transpose()?,
                declgen_v2_out_path: source_interop
                    .then(|| {
                        self.path(
                            &self
                                .input
                                .module_layout
                                .arkts_declgen_v2_dir()
                                .join(&node.package_name),
                        )
                    })
                    .transpose()?,
                declgen_bridge_code_path: source_interop
                    .then(|| self.path(&self.input.module_layout.arkts_declgen_bridge_dir()))
                    .transpose()?,
                dynamic_files,
                static_files,
                dependencies,
                original_package_name_map,
                abc_path: bytecode
                    .map(|entry| self.path(Path::new(&entry.abc_path)))
                    .transpose()?,
                byte_code_har: matches!(node.node_type, DependencyNodeType::BytecodeHar),
                bundle_type: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.bundle_type.clone()),
                bundle_name: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.bundle_name.clone()),
                package_version: (!package.version.is_empty()).then(|| package.version.clone()),
                compile_sources: matches!(
                    node.node_type,
                    DependencyNodeType::SourceHar | DependencyNodeType::OhpmSource
                ) && matches!(language.as_str(), "1.2" | "hybrid"),
            });
        }
        modules.sort_by(|left, right| {
            left.package_name
                .cmp(&right.package_name)
                .then_with(|| left.module_name.cmp(&right.module_name))
                .then_with(|| left.module_path.cmp(&right.module_path))
        });
        Ok(modules)
    }

    /// `BaseMode::getMainModuleInfo` reads the current package's dependency
    /// aliases and `originalPackageNameMap` from the main package record when
    /// it is present in `dependencyModuleList`.
    fn main_module(&self) -> Result<StaticArktsDependentModule> {
        let (dependencies, original_package_name_map) =
            self.dependency_edges(&self.input.module_context.deps.host_root);
        let entry_file = self.main_entry_file()?;
        let source_roots = if self.input.module_context.module.source_roots.is_empty() {
            vec![self.path(&self.input.module_context.module.src_main)?]
        } else {
            self.input
                .module_context
                .module
                .source_roots
                .iter()
                .map(|root| self.path(root))
                .collect::<Result<Vec<_>>>()?
        };
        let package = self.input.module_context.module.package.as_ref();
        let identity = self.main_module_identity();
        let (static_files, dynamic_files) = self.main_source_languages()?;
        Ok(StaticArktsDependentModule {
            package_name: identity.package_name().to_string(),
            module_name: identity.module_name().to_string(),
            module_type: self.input.module.module_type.to_string(),
            module_path: self.path(&self.input.module.src_path)?,
            source_roots,
            // The driver validates every dependency-list record before it
            // notices that this package is already the main module. Its
            // `demo_mix_compile2` config uses the module root as this marker;
            // the actual main entry remains the top-level `entryFile`.
            entry_file: if entry_file.is_empty() {
                self.path(&self.input.module.src_path)?
            } else {
                entry_file
            },
            language: if dynamic_files.is_empty() {
                "1.2".to_string()
            } else {
                "hybrid".to_string()
            },
            decl_files_path: (!dynamic_files.is_empty())
                .then(|| {
                    self.path(
                        &self
                            .input
                            .module_layout
                            .arkts_decl_file_info(identity.package_name()),
                    )
                })
                .transpose()?,
            declgen_v1_out_path: Some(self.path(&self.input.module_layout.arkts_declgen_v1_dir())?),
            declgen_v2_out_path: Some(
                self.path(
                    &self
                        .input
                        .module_layout
                        .arkts_declgen_v2_dir()
                        .join(identity.package_name()),
                )?,
            ),
            declgen_bridge_code_path: Some(
                self.path(&self.input.module_layout.arkts_declgen_bridge_dir())?,
            ),
            dynamic_files,
            static_files,
            dependencies,
            original_package_name_map,
            abc_path: None,
            byte_code_har: self.input.loader_json.byte_code_har.unwrap_or(false),
            bundle_type: Some(self.input.bundle_type.to_string()),
            bundle_name: Some(self.input.config.product.bundle_name.clone()),
            package_version: package
                .filter(|package| !package.version.is_empty())
                .map(|package| package.version.clone()),
            compile_sources: true,
        })
    }

    fn dependency_source_languages(
        &self,
        node: &DependencyNodeSnapshot,
        language: &str,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut static_files = Vec::new();
        let mut dynamic_files = Vec::new();
        let mut paths = node
            .compile_inputs
            .other_compile_source_files
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if let Some(entry) = node
            .compile_inputs
            .dynamic_import_entry_file
            .as_ref()
            .or(node.compile_inputs.other_compile_entry_file.as_ref())
        {
            paths.push(entry.clone());
        }
        paths.sort();
        paths.dedup();
        for path in paths {
            self.classify_source_file(&path, language, &mut static_files, &mut dynamic_files)?;
        }
        static_files.sort();
        static_files.dedup();
        dynamic_files.sort();
        dynamic_files.dedup();
        Ok((static_files, dynamic_files))
    }

    fn main_source_languages(&self) -> Result<(Vec<String>, Vec<String>)> {
        let roots = if self.input.module_context.module.source_roots.is_empty() {
            std::slice::from_ref(&self.input.module_context.module.src_main)
        } else {
            self.input.module_context.module.source_roots.as_slice()
        };
        let mut paths = roots
            .iter()
            .flat_map(|root| {
                self.input
                    .module_context
                    .session
                    .source_store
                    .source_files_under(root)
            })
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        if let Some(entry_file) = self
            .input
            .module_context
            .module
            .package
            .as_ref()
            .and_then(|package| package.package_json.main.as_deref())
        {
            let entry_file = Path::new(entry_file);
            paths.push(if entry_file.has_root() {
                lexical_normalize(entry_file)
            } else {
                lexical_normalize(&self.input.module.src_path.join(entry_file))
            });
        }
        paths.sort();
        paths.dedup();
        let mut static_files = Vec::new();
        let mut dynamic_files = Vec::new();
        for path in paths {
            self.classify_source_file(&path, "hybrid", &mut static_files, &mut dynamic_files)?;
        }
        static_files.sort();
        static_files.dedup();
        dynamic_files.sort();
        dynamic_files.dedup();
        Ok((static_files, dynamic_files))
    }

    fn classify_source_file(
        &self,
        path: &Path,
        language: &str,
        static_files: &mut Vec<String>,
        dynamic_files: &mut Vec<String>,
    ) -> Result<()> {
        if arkdown_path::SourceFileName::new(path).is_declaration() {
            return Ok(());
        }
        let extension = path.extension().and_then(|extension| extension.to_str());
        if extension == Some("ets") {
            let is_static = if language == "1.2" {
                true
            } else if language == "hybrid" {
                let source = self
                    .input
                    .module_context
                    .session
                    .source_store
                    .read_text(path)?;
                arkdown_ets_transform::ArktsSourceLanguageDetector::new(&source).is_static()
            } else {
                false
            };
            if is_static {
                static_files.push(self.path(path)?);
            } else {
                dynamic_files.push(self.path(path)?);
            }
        } else if matches!(extension, Some("ts" | "js" | "mjs" | "cjs")) {
            dynamic_files.push(self.path(path)?);
        }
        Ok(())
    }

    fn main_module_identity(&self) -> StaticArktsMainModuleIdentity<'_> {
        StaticArktsMainModuleIdentity::new(
            self.input
                .module_context
                .module
                .package
                .as_ref()
                .map(|package| package.name.as_str()),
            &self.input.loader_json.module_name,
        )
    }

    fn main_entry_file(&self) -> Result<String> {
        self.input
            .module_context
            .module
            .package
            .as_ref()
            .and_then(|package| package.package_json.main.as_deref())
            .map(Path::new)
            .map(|path| {
                if path.has_root() {
                    self.path(path)
                } else {
                    Ok(ArtifactPathSerializer::rel_to_slash(path))
                }
            })
            .unwrap_or_else(|| Ok(String::new()))
    }

    /// `BaseMode::collectModuleInfos` resolves each declared dependency alias
    /// through the module-local `originalPackageNameMap` before looking it up
    /// in `dependencyModuleList`.
    fn dependency_edges(&self, root: &Path) -> (Vec<String>, Option<BTreeMap<String, String>>) {
        let mut dependencies = Vec::new();
        let mut seen = FxHashSet::default();
        let mut original_package_name_map = BTreeMap::new();
        for edge in &self.input.module_context.deps.edges {
            if edge.issuer_root != root {
                continue;
            }
            if seen.insert(edge.alias.clone()) {
                dependencies.push(edge.alias.clone());
            }
            if edge.alias != edge.package_name {
                original_package_name_map.insert(edge.alias.clone(), edge.package_name.clone());
            }
        }
        (
            dependencies,
            (!original_package_name_map.is_empty()).then_some(original_package_name_map),
        )
    }

    /// `BaseMode::collectModuleFiles` defines this inventory only for HSPs and
    /// packages reachable from an HSP. Each record contains that package's
    /// static ArkTS source paths for `getAllModuleFilesToPathSection`.
    fn module_files(
        &self,
        dependent_modules: &[StaticArktsDependentModule],
    ) -> Result<Vec<StaticArktsModuleFile>> {
        let mut pending = self
            .input
            .module_context
            .deps
            .nodes
            .values()
            .filter(|node| {
                matches!(
                    node.node_type,
                    DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp
                )
            })
            .map(|node| node.package_name.clone())
            .collect::<Vec<_>>();
        pending.sort();
        let mut seen = FxHashSet::default();
        let mut files = Vec::new();
        while let Some(package_name) = pending.pop() {
            let Some(node) = self
                .input
                .module_context
                .deps
                .nodes
                .values()
                .find(|node| node.package_name == package_name || node.alias == package_name)
            else {
                continue;
            };
            if !seen.insert(node.package_name.clone()) {
                continue;
            }
            pending.extend(node.direct_deps.iter().cloned());

            let Some(module) = dependent_modules
                .iter()
                .find(|module| module.package_name == node.package_name)
            else {
                continue;
            };
            if module.language == "1.1" {
                continue;
            }

            let mut static_files = Vec::new();
            if let Some(entry) = node.compile_inputs.other_compile_entry_file.as_ref() {
                self.push_static_module_file(entry, &module.language, &mut static_files)?;
            }
            for path in node.compile_inputs.other_compile_source_files.iter() {
                self.push_static_module_file(path, &module.language, &mut static_files)?;
            }
            static_files.sort();
            static_files.dedup();
            if !static_files.is_empty() {
                files.push(StaticArktsModuleFile {
                    package_name: node.package_name.clone(),
                    static_files,
                });
            }
        }
        files.sort_by(|left, right| left.package_name.cmp(&right.package_name));
        Ok(files)
    }

    fn push_static_module_file(
        &self,
        path: &Path,
        language: &str,
        files: &mut Vec<String>,
    ) -> Result<()> {
        if path.extension().and_then(|extension| extension.to_str()) != Some("ets")
            || arkdown_path::SourceFileName::new(path).is_declaration()
        {
            return Ok(());
        }
        if language == "hybrid" {
            let source = self
                .input
                .module_context
                .session
                .source_store
                .read_text(path)
                .with_context(|| {
                    format!(
                        "ARKDOWN_ARKTS12_DEPENDENCY_SOURCE_NOT_SNAPSHOTTED: {}",
                        path.display()
                    )
                })?;
            if !arkdown_ets_transform::ArktsSourceLanguageDetector::new(&source)
                .is_static_dependency_compile_file()
            {
                return Ok(());
            }
        }
        files.push(self.path(path)?);
        Ok(())
    }

    fn path(&self, path: &Path) -> Result<String> {
        ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| {
            anyhow::anyhow!("serialize ArkTS 1.2 path {}: {error}", path.display())
        })
    }
}

/// Package/module identity consumed by `BaseMode::getMainModuleInfo`.
///
/// The static frontend uses `packageName` as the ABC record prefix while
/// `moduleName` remains the OHOS module name. They often both happen to be
/// `entry`, but library packages may intentionally publish a different name.
struct StaticArktsMainModuleIdentity<'a> {
    package_name: &'a str,
    module_name: &'a str,
}

impl<'a> StaticArktsMainModuleIdentity<'a> {
    fn new(package_name: Option<&'a str>, module_name: &'a str) -> Self {
        Self {
            package_name: package_name
                .filter(|name| !name.is_empty())
                .unwrap_or(module_name),
            module_name,
        }
    }

    fn package_name(&self) -> &str {
        self.package_name
    }

    fn module_name(&self) -> &str {
        self.module_name
    }
}

/// Public-entry contract for an ArkTS 1.2 ByteCode HAR.
///
/// Current `generate_arktsconfig.ts::addBinaryDependencySection` resolves a
/// binary HAR import with the fixed `mainFile: 'Index'`. The current ArkTS
/// static sample consequently publishes a module-root `Index.ets`. Accepting a
/// nested or differently named entry would produce an ABC successfully but
/// make the package impossible for the same driver to consume.
struct StaticArktsByteCodeHarEntryContract;

impl StaticArktsByteCodeHarEntryContract {
    fn validate(
        module_type: &arkdown_core::OhosModuleType,
        byte_code_har: bool,
        main: Option<&str>,
        package_name: &str,
    ) -> Result<()> {
        if !byte_code_har || !matches!(module_type, arkdown_core::OhosModuleType::Har) {
            return Ok(());
        }

        let Some(main) = main else {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_BYTECODE_HAR_ENTRY_UNSUPPORTED: package '{}' must set oh-package.json5 main to module-root Index.ets; the current static driver resolves binary HAR imports as '<package>.Index'",
                package_name
            );
        };
        let normalized = lexical_normalize(Path::new(main));
        if normalized != Path::new("Index.ets") {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_BYTECODE_HAR_ENTRY_UNSUPPORTED: package '{}' uses main '{}'; the current static driver resolves binary HAR imports as '<package>.Index', so ArkTS 1.2 ByteCode HAR requires module-root Index.ets",
                package_name,
                main
            );
        }
        Ok(())
    }
}

/// Output behavior selected by the current static driver's build modes.
struct StaticArktsModuleOutputContract {
    es2panda_mode: &'static str,
    declarations_root: Option<PathBuf>,
}

impl StaticArktsModuleOutputContract {
    fn new(
        module_type: &arkdown_core::OhosModuleType,
        layout: &ModuleBuildLayout<'_>,
        module_name: &str,
    ) -> Self {
        // `DepAnalyzer::createDependencyGraph` adds `CompileJobType.DECL` only
        // for HAR/HSP work submitted through the parallel mode. The
        // simultaneous path hard-codes an ABC-only job, so using it for a
        // library would silently leave declaration generation to an unrelated
        // producer.
        if matches!(
            module_type,
            arkdown_core::OhosModuleType::Shared | arkdown_core::OhosModuleType::Har
        ) {
            return Self {
                es2panda_mode: "parallel",
                declarations_root: Some(layout.declarations_dir().join(module_name)),
            };
        }
        Self {
            es2panda_mode: "simultaneous",
            declarations_root: None,
        }
    }
}

/// Ports language selection from
/// `bindings/src/common/utils.ts::getFileLanguageVersion` and
/// `bindings/src/lsp/generateBuildConfig.ts::getModuleLanguageVersion`.
struct StaticArktsLanguageDetector<'a> {
    source_store: &'a SourceStore,
}

impl<'a> StaticArktsLanguageDetector<'a> {
    fn new(source_store: &'a SourceStore) -> Self {
        Self { source_store }
    }

    fn detect(
        &self,
        node: &DependencyNodeSnapshot,
        bytecode_abc_path: Option<&String>,
    ) -> Result<String> {
        if matches!(node.node_type, DependencyNodeType::BytecodeHar) {
            let path = bytecode_abc_path.ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_BYTECODE_DEPENDENCY_METADATA_MISSING: {}",
                    node.package_name
                )
            })?;
            return match Path::new(path).file_name().and_then(|name| name.to_str()) {
                Some("modules_static.abc") => Ok("1.2".to_string()),
                Some("modules.abc") => Ok("1.1".to_string()),
                _ => anyhow::bail!(
                    "ARKDOWN_ARKTS12_BYTECODE_LANGUAGE_UNKNOWN: package '{}' uses {}",
                    node.package_name,
                    path
                ),
            };
        }

        let mut found_dynamic = false;
        let mut found_static = false;
        for path in node
            .compile_inputs
            .source_inventory_files
            .iter()
            .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("ets"))
        {
            let source = self.source_store.read_text(path).with_context(|| {
                format!(
                    "ARKDOWN_ARKTS12_DEPENDENCY_SOURCE_NOT_SNAPSHOTTED: {}",
                    path.display()
                )
            })?;
            if arkdown_ets_transform::ArktsSourceLanguageDetector::new(&source).is_static() {
                found_static = true;
            } else {
                found_dynamic = true;
            }
            if found_dynamic && found_static {
                return Ok("hybrid".to_string());
            }
        }
        Ok(if found_static { "1.2" } else { "1.1" }.to_string())
    }
}

struct StaticArktsCompileEntryPlan<'a> {
    dependent_modules: &'a [StaticArktsDependentModule],
}

impl<'a> StaticArktsCompileEntryPlan<'a> {
    fn new(dependent_modules: &'a [StaticArktsDependentModule]) -> Self {
        Self { dependent_modules }
    }

    fn resolve(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut seen = FxHashSet::default();
        for module in self.dependent_modules {
            if !module.compile_sources {
                continue;
            }
            for static_file in &module.static_files {
                let path = lexical_normalize(Path::new(static_file));
                if !path.is_file() {
                    anyhow::bail!("ARKDOWN_ARKTS12_COMPILE_ENTRY_MISSING: {}", path.display());
                }
                if seen.insert(path.clone()) {
                    files.push(path);
                }
            }
        }
        Ok(files)
    }
}

struct StaticArktsBuildMode<'a> {
    mode: &'a BuildMode,
}

impl<'a> StaticArktsBuildMode<'a> {
    fn new(mode: &'a BuildMode) -> Self {
        Self { mode }
    }
}

impl std::fmt::Display for StaticArktsBuildMode<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.mode {
            BuildMode::Debug => "Debug",
            BuildMode::Release => "Release",
            BuildMode::Test => "Test",
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDependentModule {
    package_name: String,
    module_name: String,
    module_type: String,
    module_path: String,
    source_roots: Vec<String>,
    entry_file: String,
    language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    decl_files_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    declgen_v1_out_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    declgen_v2_out_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    declgen_bridge_code_path: Option<String>,
    dynamic_files: Vec<String>,
    static_files: Vec<String>,
    dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_package_name_map: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    abc_path: Option<String>,
    byte_code_har: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_version: Option<String>,
    #[serde(skip)]
    compile_sources: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDynamicDeclgenRequest<'a> {
    modules: &'a [StaticArktsDependentModule],
    cache_path: &'a str,
    bundle_name: &'a str,
    main_module_name: &'a str,
    project_root_path: &'a Path,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDeclFileIndex<'a> {
    package_name: &'a str,
    files: BTreeMap<String, StaticArktsDeclFileRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDeclFileRecord {
    decl_path: String,
    file_path: String,
    ohm_url: String,
}

struct StaticArktsInteropPath;

impl StaticArktsInteropPath {
    fn without_source_extension(path: &Path) -> Result<String> {
        let serialized = ArtifactPathSerializer::rel_to_slash(path);
        for suffix in [".d.ets", ".d.ts", ".ets", ".ts", ".mjs", ".cjs", ".js"] {
            if let Some(value) = serialized.strip_suffix(suffix) {
                return Ok(value.to_string());
            }
        }
        anyhow::bail!(
            "ARKDOWN_ARKTS12_INTEROP_SOURCE_EXTENSION: unsupported source {}",
            path.display()
        )
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDriverConfig {
    plugins: IndexMap<String, String>,
    paths: BTreeMap<String, Vec<String>>,
    entry_file: String,
    compile_files: Vec<String>,
    module_files: Vec<StaticArktsModuleFile>,
    build_mode: String,
    project_root_path: String,
    module_root_path: String,
    ace_profile_path: String,
    cache_path: String,
    loader_out_path: String,
    compile_sdk_version: u32,
    compatible_sdk_version: u32,
    ace_module_json_path: String,
    bundle_name: String,
    use_normalized_ohm_url: bool,
    build_type: String,
    package_name: String,
    build_sdk_path: String,
    source_roots: Vec<String>,
    module_type: String,
    module_name: String,
    dependent_module_list: Vec<StaticArktsDependentModule>,
    dependency_module_list: Vec<StaticArktsDependentModule>,
    has_main_module: bool,
    build_loader_json: String,
    integrated_hsp: bool,
    allow_empty_bundle_name: bool,
    enable_declgen_ets2_ts: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    declgen_v1_out_path: Option<String>,
    declgen_v2_out_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    declgen_bridge_code_path: Option<String>,
    external_api_paths: Vec<String>,
    interop_api_paths: Vec<String>,
    device_types: Vec<String>,
    byte_code_har: bool,
    ui_transform_optimization: bool,
    #[serde(rename = "runtimeOS")]
    runtime_os: String,
    is_preview: bool,
    check_entry: bool,
    code_root_path: String,
    is_crossplatform: bool,
    bundle_type: String,
    level: StaticArktsLogLevel,
    is_build_config_modified: bool,
    panda_sdk_path: String,
    abc_linker_path: String,
    dependency_analyzer_path: String,
    api_path: String,
    kits_path: String,
    dep_analyzer_path: String,
    sdk_alias_map: BTreeMap<String, serde_json::Value>,
    es2panda_mode: String,
    app_resource: String,
    raw_file_resource: String,
    project_path: String,
    hsp_resources_map: bool,
    compile_har: bool,
    reset_bundle_name: bool,
    ignore_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ark_guard_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    obfuscation_options: Option<StaticArktsDriverObfuscationOptions>,
}

impl StaticArktsDriverConfig {
    fn has_static_output_inputs(&self) -> bool {
        !self.compile_files.is_empty()
            || self.dependency_module_list.iter().any(|module| {
                module.byte_code_har && module.language != "1.1" && module.abc_path.is_some()
            })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDriverObfuscationOptions {
    self_config: StaticArktsDriverSelfObfuscation,
    sdk_apis: Vec<String>,
    obfuscation_cache_dir: String,
    export_rule_path: String,
    dependencies: StaticArktsDriverObfuscationDependencies,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDriverSelfObfuscation {
    rule_options: StaticArktsDriverRuleOptions,
    consumer_rules: Vec<String>,
    lib_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDriverRuleOptions {
    enable: bool,
    rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDriverObfuscationDependencies {
    libraries: Vec<StaticArktsDriverDependencyObfuscation>,
    hsp_libraries: Vec<StaticArktsDriverDependencyObfuscation>,
    hars: Vec<String>,
    hsps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDriverDependencyObfuscation {
    consumer_rules: Vec<String>,
    lib_dir: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsGeneratedObfuscationConfig {
    abc_path: String,
    obf_abc_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsModuleFile {
    package_name: String,
    static_files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsLogLevel {
    level: u32,
    level_str: String,
    colour: String,
}

impl Default for StaticArktsLogLevel {
    fn default() -> Self {
        Self {
            level: 20_000,
            level_str: "INFO".to_string(),
            colour: "green".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use arkdown_core::OhosModuleType;

    use super::{
        ModuleBuildLayout, StaticArktsByteCodeHarEntryContract, StaticArktsCompileEntryPlan,
        StaticArktsDependentModule, StaticArktsInteropOutput, StaticArktsInteropPath,
        StaticArktsMainModuleIdentity, StaticArktsModuleOutputContract, StaticArktsObfuscationPlan,
    };

    fn compile_module(
        root: &std::path::Path,
        static_files: Vec<String>,
        dynamic_files: Vec<String>,
    ) -> StaticArktsDependentModule {
        StaticArktsDependentModule {
            package_name: "entry".to_string(),
            module_name: "entry".to_string(),
            module_type: "entry".to_string(),
            module_path: root.display().to_string(),
            source_roots: vec![root.display().to_string()],
            entry_file: root.display().to_string(),
            language: if dynamic_files.is_empty() {
                "1.2".to_string()
            } else {
                "hybrid".to_string()
            },
            decl_files_path: None,
            declgen_v1_out_path: None,
            declgen_v2_out_path: None,
            declgen_bridge_code_path: None,
            dynamic_files,
            static_files,
            dependencies: Vec::new(),
            original_package_name_map: None,
            abc_path: None,
            byte_code_har: false,
            bundle_type: None,
            bundle_name: None,
            package_version: None,
            compile_sources: true,
        }
    }

    fn static_sdk(root: &std::path::Path) -> arkdown_core::StaticArktsSdkLayout {
        let ets = root.join("ets");
        let dynamic_loader = ets.join("dynamic/build-tools/ets-loader");
        std::fs::create_dir_all(&dynamic_loader).expect("dynamic SDK loader");
        arkdown_config::sdk::sdk_config_from_root(root, Some(26))
            .static_arkts_layout()
            .expect("static SDK layout")
    }

    #[test]
    fn language_directive_matches_the_frontend_first_effective_line_algorithm() {
        assert!(arkdown_ets_transform::ArktsSourceLanguageDetector::new(
            "// heading\n/* block\n * comment\n */ 'use static'\nexport class A {}"
        )
        .is_static());
        assert!(arkdown_ets_transform::ArktsSourceLanguageDetector::new(
            "/* before */ 'use static' // after"
        )
        .is_static());
        assert!(!arkdown_ets_transform::ArktsSourceLanguageDetector::new(
            "export const value = 1;\n'use static'"
        )
        .is_static());
    }

    #[test]
    fn interop_paths_follow_the_sdk_declgen_output_contract() {
        assert_eq!(
            StaticArktsInteropPath::without_source_extension(std::path::Path::new(
                "src/main/ets/model/Index.ets"
            ))
            .expect("source path"),
            "src/main/ets/model/Index"
        );
        assert_eq!(
            StaticArktsInteropPath::without_source_extension(std::path::Path::new(
                "src/main/ets/model/Types.d.ts"
            ))
            .expect("declaration path"),
            "src/main/ets/model/Types"
        );
    }

    #[test]
    fn interop_entry_redirect_preserves_the_entry_query() {
        let root = std::path::PathBuf::from("/project/entry");
        let source = root.join("src/main/ets/pages/Index.ets");
        let bridge = std::path::PathBuf::from(
            "/project/entry/build/default/intermediates/declgen/default/declgenBridgeCode/entry/src/main/ets/pages/Index.ts",
        );
        let output = StaticArktsInteropOutput {
            bridge_root: bridge.parent().expect("bridge parent").to_path_buf(),
            source_redirects: [(source, bridge.clone())].into_iter().collect(),
            requires_dynamic_abc: true,
        };
        let mut entries = vec!["./src/main/ets/pages/Index.ets?entry".to_string()];

        output
            .redirect_entries(&root, &mut entries)
            .expect("redirect entry");

        assert_eq!(entries, vec![format!("{}?entry", bridge.display())]);
    }

    #[test]
    fn static_abc_identity_uses_package_name_not_ohos_module_name() {
        let identity = StaticArktsMainModuleIdentity::new(Some("bytecode-har"), "entry");

        assert_eq!(identity.package_name(), "bytecode-har");
        assert_eq!(identity.module_name(), "entry");
        assert_eq!(
            StaticArktsMainModuleIdentity::new(None, "entry").package_name(),
            "entry"
        );
    }

    #[test]
    fn static_bytecode_har_requires_the_driver_binary_index_contract() {
        StaticArktsByteCodeHarEntryContract::validate(
            &OhosModuleType::Har,
            true,
            Some("./Index.ets"),
            "library",
        )
        .expect("module-root Index.ets is consumable as library.Index");

        let error = StaticArktsByteCodeHarEntryContract::validate(
            &OhosModuleType::Har,
            true,
            Some("src/main/ets/Index.ets"),
            "library",
        )
        .expect_err("nested main cannot satisfy the current binary HAR mainFile contract");
        assert!(
            error
                .to_string()
                .contains("ARKDOWN_ARKTS12_BYTECODE_HAR_ENTRY_UNSUPPORTED"),
            "{error}"
        );

        StaticArktsByteCodeHarEntryContract::validate(
            &OhosModuleType::Har,
            false,
            Some("src/main/ets/Index.ets"),
            "source-library",
        )
        .expect("source HARs retain explicit nested entry mapping");
    }

    #[test]
    fn compile_entries_contain_only_the_classified_static_subset_and_deduplicate() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("src/main");
        std::fs::create_dir_all(root.join("ets/pages")).expect("source directory");
        let page = root.join("ets/pages/Index.ets");
        let ability = root.join("ets/EntryAbility.ets");
        std::fs::write(&page, "").expect("page source");
        std::fs::write(&ability, "").expect("ability source");
        let modules = [compile_module(
            &root,
            vec![
                ability.display().to_string(),
                page.display().to_string(),
                ability.display().to_string(),
            ],
            Vec::new(),
        )];

        let resolved = StaticArktsCompileEntryPlan::new(&modules)
            .resolve()
            .expect("static compile entry plan");

        assert_eq!(resolved, [ability, page]);
    }

    #[test]
    fn compile_entries_exclude_dynamic_ets_and_support_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("src/main");
        let generated = temp.path().join("build/generated/profile/default");
        std::fs::create_dir_all(root.join("ets")).expect("source directory");
        std::fs::create_dir_all(&generated).expect("generated directory");
        let static_index = root.join("ets/StaticIndex.ets");
        let dynamic_index = root.join("ets/DynamicIndex.ets");
        let module_info = generated.join("ModuleInfo.ts");
        std::fs::write(&static_index, "'use static';\nexport const value = 1;\n")
            .expect("static source");
        std::fs::write(&dynamic_index, "export const value = 1;\n").expect("dynamic source");
        std::fs::write(
            &module_info,
            "export const __MODULE_NAME__ = __getCurrentModuleName__();\n",
        )
        .expect("dynamic support source");
        let modules = [compile_module(
            &root,
            vec![static_index.display().to_string()],
            vec![
                dynamic_index.display().to_string(),
                module_info.display().to_string(),
            ],
        )];

        let resolved = StaticArktsCompileEntryPlan::new(&modules)
            .resolve()
            .expect("static compile entry plan");

        assert_eq!(resolved, [static_index]);
    }

    #[test]
    fn compile_entries_allow_an_arkts12_product_with_only_dynamic_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dynamic_index = temp.path().join("ets/DynamicIndex.ets");
        std::fs::create_dir_all(dynamic_index.parent().expect("source parent"))
            .expect("source directory");
        std::fs::write(&dynamic_index, "export const value = 1;\n").expect("dynamic source");
        let modules = [compile_module(
            temp.path(),
            Vec::new(),
            vec![dynamic_index.display().to_string()],
        )];

        let resolved = StaticArktsCompileEntryPlan::new(&modules)
            .resolve()
            .expect("an empty static partition is valid");

        assert!(resolved.is_empty());
    }

    #[test]
    fn missing_static_compile_entry_is_a_typed_boundary_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let modules = [compile_module(
            temp.path(),
            vec![temp
                .path()
                .join("ets/pages/Missing.ets")
                .display()
                .to_string()],
            Vec::new(),
        )];
        let error = StaticArktsCompileEntryPlan::new(&modules)
            .resolve()
            .expect_err("missing static root must be rejected before launching Node");

        assert!(
            error
                .to_string()
                .contains("ARKDOWN_ARKTS12_COMPILE_ENTRY_MISSING"),
            "{error}"
        );
    }

    #[test]
    fn hsp_and_har_use_parallel_declgen_contract() {
        let module_root = std::path::Path::new("/project/library");
        let layout = ModuleBuildLayout::new(module_root);
        for module_type in [OhosModuleType::Shared, OhosModuleType::Har] {
            let contract = StaticArktsModuleOutputContract::new(&module_type, &layout, "library");
            assert_eq!(contract.es2panda_mode, "parallel");
            assert_eq!(
                contract.declarations_root,
                Some(
                    module_root
                        .join("build/default/intermediates/loader_out/default/etsFortgz/library")
                )
            );
        }
    }

    #[test]
    fn hap_uses_simultaneous_abc_without_declgen_output() {
        let layout = ModuleBuildLayout::new(std::path::Path::new("/project/entry"));
        let contract =
            StaticArktsModuleOutputContract::new(&OhosModuleType::Entry, &layout, "entry");
        assert_eq!(contract.es2panda_mode, "simultaneous");
        assert_eq!(contract.declarations_root, None);
    }

    #[test]
    fn static_obfuscation_handoff_matches_driver_option_shape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sdk = static_sdk(temp.path());
        let rule = temp.path().join("rules.txt");
        let consumer = temp.path().join("consumer.txt");
        let dependency = temp.path().join("dependency.txt");
        std::fs::write(
            &rule,
            "-enable-bytecode-obfuscation\n-enable-bytecode-obfuscation-debugging\n",
        )
        .expect("rule file");
        std::fs::write(&consumer, "-keep class Public {*;}\n").expect("consumer rule");
        std::fs::write(&dependency, "-keep class Dependency {*;}\n").expect("dependency rule");
        let cache = temp.path().join("cache/obfuscation");

        let plan = StaticArktsObfuscationPlan::new(
            &sdk,
            std::slice::from_ref(&rule),
            std::slice::from_ref(&consumer),
            std::slice::from_ref(&dependency),
            cache.clone(),
        )
        .expect("static obfuscation selection")
        .expect("static obfuscation plan");
        let value = serde_json::to_value(plan.driver_options()).expect("serialize options");

        assert_eq!(value["selfConfig"]["ruleOptions"]["enable"], true);
        assert_eq!(
            value["selfConfig"]["ruleOptions"]["rules"][0],
            arkdown_path::ArtifactPathSerializer::path_to_slash_string(&rule)
                .unwrap()
                .as_str()
        );
        assert_eq!(
            value["selfConfig"]["consumerRules"][0],
            arkdown_path::ArtifactPathSerializer::path_to_slash_string(&consumer)
                .unwrap()
                .as_str()
        );
        assert_eq!(
            value["dependencies"]["hars"][0],
            arkdown_path::ArtifactPathSerializer::path_to_slash_string(&dependency)
                .unwrap()
                .as_str()
        );
        assert_eq!(
            value["obfuscationCacheDir"],
            arkdown_path::ArtifactPathSerializer::path_to_slash_string(&cache)
                .unwrap()
                .as_str()
        );
        assert_eq!(
            value["sdkApis"],
            serde_json::json!([
                sdk.kits.to_string_lossy(),
                sdk.api.to_string_lossy(),
                sdk.arkts.to_string_lossy()
            ])
        );
        assert!(plan.debugging);
    }

    #[cfg(unix)]
    #[test]
    fn static_obfuscation_executes_sdk_guard_and_replaces_modules_static_abc() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let sdk = static_sdk(temp.path());
        std::fs::create_dir_all(sdk.ark_guard.parent().expect("guard parent"))
            .expect("guard directory");
        let cache = temp.path().join("cache/obfuscation");
        let obfuscated = cache.join("obf/modules_static.abc");
        std::fs::create_dir_all(obfuscated.parent().expect("obfuscated parent"))
            .expect("obfuscated directory");
        let modules_abc = temp.path().join("loader/modules_static.abc");
        std::fs::create_dir_all(modules_abc.parent().expect("abc parent")).expect("abc directory");
        std::fs::write(&modules_abc, b"original-static-abc").expect("input ABC");
        let script = format!(
            "#!/bin/sh\nprintf obfuscated-static-abc > '{}'\n",
            obfuscated.display()
        );
        std::fs::write(&sdk.ark_guard, script).expect("fake ark_guard");
        let mut permissions = std::fs::metadata(&sdk.ark_guard)
            .expect("guard metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&sdk.ark_guard, permissions).expect("executable guard");
        std::fs::create_dir_all(&cache).expect("obfuscation cache");
        std::fs::write(
            cache.join("config.json"),
            serde_json::to_vec(&serde_json::json!({
                "abcPath": modules_abc,
                "obfAbcPath": obfuscated,
                "defaultNameCachePath": cache.join("nameCache.json"),
                "obfuscationRules": {}
            }))
            .expect("config JSON"),
        )
        .expect("driver-generated config");
        let rule = temp.path().join("rules.txt");
        std::fs::write(&rule, "-enable-bytecode-obfuscation\n").expect("rule file");
        let plan = StaticArktsObfuscationPlan::new(
            &sdk,
            std::slice::from_ref(&rule),
            &[],
            &[],
            cache.clone(),
        )
        .expect("static obfuscation selection")
        .expect("static obfuscation plan");

        plan.execute(&modules_abc).expect("execute static guard");

        assert_eq!(
            std::fs::read(&modules_abc).expect("replaced ABC"),
            b"obfuscated-static-abc"
        );
        assert_eq!(
            std::fs::read(cache.join("origin/modules_static.abc")).expect("origin ABC"),
            b"original-static-abc"
        );
    }

    #[test]
    fn dependency_rules_cannot_select_or_disable_static_bytecode_obfuscation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sdk = static_sdk(temp.path());
        let source_rule = temp.path().join("source-rules.txt");
        let dependency_rule = temp.path().join("dependency-rules.txt");
        std::fs::write(&source_rule, "-enable-property-obfuscation\n").expect("source rule");
        std::fs::write(
            &dependency_rule,
            "-enable-bytecode-obfuscation\n-disable-obfuscation\n",
        )
        .expect("dependency rule");

        let plan = StaticArktsObfuscationPlan::new(
            &sdk,
            std::slice::from_ref(&source_rule),
            &[],
            std::slice::from_ref(&dependency_rule),
            temp.path().join("cache"),
        )
        .expect("static obfuscation selection");

        assert!(plan.is_none());
    }

    #[test]
    fn self_disable_suppresses_static_bytecode_obfuscation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let sdk = static_sdk(temp.path());
        let rule = temp.path().join("rules.txt");
        std::fs::write(
            &rule,
            "-enable-bytecode-obfuscation\n-disable-obfuscation\n",
        )
        .expect("rule file");

        let plan = StaticArktsObfuscationPlan::new(
            &sdk,
            std::slice::from_ref(&rule),
            &[],
            &[],
            temp.path().join("cache"),
        )
        .expect("static obfuscation selection");

        assert!(plan.is_none());
    }
}
