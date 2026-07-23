//! `arkdown-profile` — module.json5 processor and loader.json generator.
//!
//! This crate:
//! 1. Transforms a raw [`ModuleJson5`] into a processed [`HapManifest`]
//!    (sets `virtualMachine`, `compileMode`, maps abilities, etc.).
//! 2. Generates [`LoaderJson`] — the manifest the bundler reads for entry points
//!    and module metadata.
//!
//! ## Primary API
//! Use [`ProfileProcessor`] as the top-level driver:
//! ```rust,ignore
//! let processor = ProfileProcessor::new(Arc::new(config));
//! let (manifest, loader) = processor.process_module(&module_cfg, &module_json5)?;
//! processor.write_loader_json(&loader, Path::new("/module/build/loader.json"))?;
//! ```

pub mod bytecode_har;
pub mod pkg_context_info;
pub mod plan;
// Per SPEC-048 Phase 2A: `lockfile` and `ohpm_registry` have been deleted
// from arkdown-profile. Their functionality is now owned by `arkdown-ohpm`.
//
// `dependency_graph` is kept `pub(crate)` during Phase 3 migration; it still
// provides `LocalDependencyGraph` used by internal profile processors.
// It will move to `arkdown-session::dependency_graph` in Phase 3.
pub(crate) mod dependency_graph;
pub mod form_profile;
pub mod har_abilities;
pub(crate) mod layout;
pub mod loader_json;
pub(crate) mod module_json_writer;
pub mod process_distro_filter;
pub mod process_permission_reason;
pub mod process_privacy_profile;
pub mod process_profile;
pub mod process_share_config;
pub mod process_shortcut_profile;
pub mod process_startup_config;
pub(crate) mod resource_ref;
pub mod router_map;
pub mod validate_permissions;

pub use arkdown_core::PkgBriefInfo;
pub use bytecode_har::{
    ByteCodeHarMetadataWriter, ByteCodeHarScanEntry, CompatibleSdkVersionValue,
    DynamicImportLibEntry, HostModuleInfo, LoaderByteCodeHarFields,
};
pub use form_profile::{
    FormProfileEntry, FormProfileInput, FormProfileProcessor, FormSourceEntry,
    ModuleFormProfileInput,
};
pub use har_abilities::{HarAbility, HarAbilityCollector};
pub use loader_json::{ByteCodeHarEntry, LoaderJson};
pub use process_distro_filter::{
    DistroFilterProcessor, DistroFilterProfileInput, DistroFilterProfileOutput,
};
pub use process_permission_reason::{
    DependencyPermissionReasonGraphResourceInput, DependencyPermissionReasonResourceInput,
    PermissionReasonProcessor, PermissionReasonResourceInput,
};
pub use process_privacy_profile::{
    PrivacyProfileInput, PrivacyProfileOutput, PrivacyProfileProcessor, ProjectPrivacyProfileInput,
    ProjectPrivacyProfileOutput,
};
pub use process_profile::ProcessProfileProcessor;
pub use process_share_config::{
    ShareConfigDependencyInput, ShareConfigInput, ShareConfigOutput, ShareConfigProcessor,
};
pub use process_shortcut_profile::{ShortcutProfileInput, ShortcutProfileProcessor};
pub use process_startup_config::{StartupConfigInput, StartupConfigOutput, StartupConfigProcessor};
pub use router_map::{RouterMapEntry, RouterMapProcessor};
pub use validate_permissions::{
    PermissionValidator, RequestPermissionValidationInput, SdkPermissionData,
};

// SPEC-048 Phase 3: typed plan inputs (re-export from plan.rs)
pub use pkg_context_info::{
    PkgContextCurrentInput, PkgContextDependencyInput, PkgContextDependencyKind,
    PkgContextInfoGenerator, PkgContextInfoInput, PkgContextInfoOpt, PkgContextModuleKind,
};
pub use plan::{
    LoaderJsonPlan, PackageModuleJsonPlan, PkgContextInfoPlan, ProfileArtifactPaths,
    ProfileProcessPlan,
};

use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use arkdown_config::{build_profile::parse_api_version, sdk::discover_ark_vm_version};
use arkdown_core::{
    ArkdownConfig, ArkdownError, CompileMode, HapManifest, ModuleConfig, ModuleJson5,
};
use module_json_writer::ModuleJsonWriter;

/// Top-level driver for profile processing and loader.json generation.
///
/// Holds a shared reference to the project-wide [`ArkdownConfig`] so it can be
/// reused across multiple modules without cloning.
pub struct ProfileProcessor {
    config: Arc<ArkdownConfig>,
}

pub struct ProfileDependencyContext {
    graph: dependency_graph::LocalDependencyGraph,
    hsp_dependencies: Vec<arkdown_core::HspDependency>,
    app_version_code: u64,
}

impl ProfileDependencyContext {
    fn build(config: &ArkdownConfig, module_cfg: &ModuleConfig) -> Self {
        let graph = dependency_graph::LocalDependencyGraph::build(
            &module_cfg.src_path,
            &config.project_root,
        );
        let app_version_code = config.product.version_code.unwrap_or_default();
        let hsp_dependencies = Self::hsp_dependencies_from_local_graph(&graph, app_version_code);
        Self {
            graph,
            hsp_dependencies,
            app_version_code,
        }
    }

    pub fn from_session_graph(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        session_graph: &arkdown_session::DependencyGraphSnapshot,
        app_version_code: u64,
    ) -> Self {
        let mut graph = dependency_graph::LocalDependencyGraph {
            nodes: indexmap::IndexMap::new(),
        };
        for node in session_graph.nodes.values() {
            let package_name = node.alias.clone();
            graph.nodes.insert(
                package_name.clone(),
                dependency_graph::DependencyNode {
                    package_name,
                    pkg_root: node.package_root.clone().unwrap_or_default(),
                    node_type: Self::session_node_type(node),
                },
            );
        }
        let hsp_dependencies = session_graph
            .nodes
            .values()
            .filter(|node| {
                matches!(
                    node.node_type,
                    arkdown_session::dependency_graph::snapshot::DependencyNodeType::SharedHsp
                        | arkdown_session::dependency_graph::snapshot::DependencyNodeType::IntegratedHsp
                )
            })
            .map(|node| {
                let profile = node.module_profile.as_ref();
                let has_common_app_identity = profile
                    .and_then(|profile| profile.bundle_type.as_deref())
                    == Some("shared");
                arkdown_core::HspDependency {
                    module_name: profile
                        .and_then(|profile| profile.module_name.clone())
                        .unwrap_or_default(),
                    version_code: profile
                        .and_then(|profile| {
                            profile
                                .bundle_name
                                .as_deref()
                                .filter(|bundle_name| !bundle_name.is_empty())
                                .and(profile.version_code)
                        })
                        .unwrap_or(app_version_code),
                    bundle_name: has_common_app_identity
                        .then(|| profile.and_then(|profile| profile.bundle_name.clone()))
                        .flatten(),
                }
            })
            .collect();
        let mut context = Self {
            graph,
            hsp_dependencies,
            app_version_code,
        };
        context.apply_root_runtime_only_packages(config, module_cfg);
        context
    }

    pub(crate) fn graph(&self) -> &dependency_graph::LocalDependencyGraph {
        &self.graph
    }

    fn hsp_dependencies(&self) -> &[arkdown_core::HspDependency] {
        &self.hsp_dependencies
    }

    fn hsp_dependencies_from_local_graph(
        graph: &dependency_graph::LocalDependencyGraph,
        app_version_code: u64,
    ) -> Vec<arkdown_core::HspDependency> {
        graph
            .shared_hsps()
            .into_iter()
            .filter_map(|node| {
                let profile_path = [
                    node.pkg_root.join("src/main/module.json"),
                    node.pkg_root.join("src/main/module.json5"),
                ]
                .into_iter()
                .find(|path| path.is_file())?;
                let content = fs_err::read_to_string(profile_path).ok()?;
                let profile: serde_json::Value = json5::from_str(&content).ok()?;
                let module_name = profile.pointer("/module/name")?.as_str()?.to_string();
                let common_app =
                    profile.pointer("/app/bundleType").and_then(|v| v.as_str()) == Some("shared");
                Some(arkdown_core::HspDependency {
                    module_name,
                    version_code: profile
                        .pointer("/app/bundleName")
                        .and_then(|value| value.as_str())
                        .filter(|bundle_name| !bundle_name.is_empty())
                        .and_then(|_| profile.pointer("/app/versionCode"))
                        .and_then(|value| value.as_u64())
                        .unwrap_or(app_version_code),
                    bundle_name: common_app
                        .then(|| {
                            profile
                                .pointer("/app/bundleName")
                                .and_then(|value| value.as_str())
                                .map(str::to_string)
                        })
                        .flatten(),
                })
            })
            .collect()
    }

    /// Remove dependency records owned by the selected package roots.
    ///
    /// Hvigor applies this to HSP profile processing when `deduplicateHar` is
    /// enabled so abilities and other HAR-contributed module metadata are
    /// emitted by the merged HAP only.
    pub fn excluding_package_roots(mut self, roots: &[PathBuf]) -> Self {
        let roots = roots
            .iter()
            .map(|root| arkdown_path::canonicalize_or_owned(root))
            .collect::<rustc_hash::FxHashSet<_>>();
        self.graph.nodes.retain(|_, node| {
            !roots.contains(&arkdown_path::canonicalize_or_owned(&node.pkg_root))
        });
        self
    }

    fn session_node_type(
        node: &arkdown_session::dependency_graph::snapshot::DependencyNodeSnapshot,
    ) -> dependency_graph::DependencyNodeType {
        if node.is_runtime_only {
            return dependency_graph::DependencyNodeType::RuntimeOnly;
        }
        match node.node_type {
            arkdown_session::dependency_graph::snapshot::DependencyNodeType::SourceHar => {
                dependency_graph::DependencyNodeType::SourceHar
            }
            arkdown_session::dependency_graph::snapshot::DependencyNodeType::BytecodeHar => {
                dependency_graph::DependencyNodeType::BytecodeHar
            }
            arkdown_session::dependency_graph::snapshot::DependencyNodeType::SharedHsp
            | arkdown_session::dependency_graph::snapshot::DependencyNodeType::IntegratedHsp => {
                dependency_graph::DependencyNodeType::SharedHsp
            }
            arkdown_session::dependency_graph::snapshot::DependencyNodeType::NativeSo => {
                dependency_graph::DependencyNodeType::NativeSo
            }
            arkdown_session::dependency_graph::snapshot::DependencyNodeType::OhpmSource
            | arkdown_session::dependency_graph::snapshot::DependencyNodeType::Unknown => {
                dependency_graph::DependencyNodeType::RemoteOrUnknown
            }
        }
    }

    fn apply_root_runtime_only_packages(
        &mut self,
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
    ) {
        let mut runtime_only_packages = Self::runtime_only_package_names(&module_cfg.src_path);
        runtime_only_packages.extend(Self::runtime_only_package_names(&config.project_root));
        runtime_only_packages.sort();
        runtime_only_packages.dedup();
        for package_name in runtime_only_packages {
            if let Some(node) = self.graph.nodes.get_mut(&package_name) {
                node.node_type = dependency_graph::DependencyNodeType::RuntimeOnly;
                continue;
            }
            let pkg_root = Self::resolve_runtime_only_root(
                &module_cfg.src_path,
                &config.project_root,
                &package_name,
            )
            .unwrap_or_default();
            self.graph.nodes.insert(
                package_name.clone(),
                dependency_graph::DependencyNode {
                    package_name,
                    pkg_root,
                    node_type: dependency_graph::DependencyNodeType::RuntimeOnly,
                },
            );
        }
    }

    fn runtime_only_package_names(package_root: &Path) -> Vec<String> {
        let pkg_json = package_root.join("oh-package.json5");
        let Ok(content) = fs_err::read_to_string(&pkg_json) else {
            return Vec::new();
        };
        #[derive(serde::Deserialize)]
        struct Pkg {
            #[serde(default)]
            metadata: PkgMetaRuntime,
        }
        #[derive(Default, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PkgMetaRuntime {
            #[serde(default)]
            runtime_only: RuntimeOnlySection,
        }
        #[derive(Default, serde::Deserialize)]
        struct RuntimeOnlySection {
            #[serde(default)]
            packages: Vec<String>,
        }
        json5::from_str::<Pkg>(&content)
            .map(|pkg| pkg.metadata.runtime_only.packages)
            .unwrap_or_default()
    }

    fn resolve_runtime_only_root(
        module_root: &Path,
        project_root: &Path,
        package_name: &str,
    ) -> Option<PathBuf> {
        for base in [
            module_root.join("oh_modules"),
            project_root.join("oh_modules"),
        ] {
            let candidate = base.join(package_name);
            if candidate.is_dir() {
                return Some(arkdown_path::canonicalize_or_owned(&candidate));
            }
        }
        None
    }
}

struct ProfileTimingProbe<'a> {
    module: &'a str,
    stage: &'static str,
    start: Instant,
}

impl<'a> ProfileTimingProbe<'a> {
    fn new(module: &'a str, stage: &'static str) -> Self {
        Self {
            module,
            stage,
            start: Instant::now(),
        }
    }
}

impl Drop for ProfileTimingProbe<'_> {
    fn drop(&mut self) {
        tracing::debug!(
            module = %self.module,
            stage = self.stage,
            elapsed_ms = self.start.elapsed().as_millis(),
            "profile processor substage completed"
        );
    }
}

impl ProfileProcessor {
    /// Create a new processor for the given project configuration.
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        ProfileProcessor { config }
    }

    pub fn dependency_context(&self, module_cfg: &ModuleConfig) -> ProfileDependencyContext {
        ProfileDependencyContext::build(&self.config, module_cfg)
    }

    /// Process a single module: transform its `module.json5` into a
    /// [`HapManifest`] and generate a corresponding [`LoaderJson`].
    ///
    /// # Parameters
    /// - `module_cfg` — the module entry from [`ArkdownConfig::modules`]
    /// - `module_json5` — the parsed `module.json5` for this module
    pub fn process_module(
        &self,
        module_cfg: &ModuleConfig,
        module_json5: &ModuleJson5,
    ) -> Result<(HapManifest, LoaderJson), ArkdownError> {
        let dependency_context = self.dependency_context(module_cfg);
        self.process_module_with_dependency_context(module_cfg, module_json5, &dependency_context)
    }

    pub fn process_module_with_dependency_context(
        &self,
        module_cfg: &ModuleConfig,
        module_json5: &ModuleJson5,
        dependency_context: &ProfileDependencyContext,
    ) -> Result<(HapManifest, LoaderJson), ArkdownError> {
        let discovered_ark_vm_version = {
            let _timer = ProfileTimingProbe::new(&module_cfg.name, "discover_ark_vm_version");
            let profile_api_version =
                parse_api_version(&self.config.product.compatible_sdk_version)
                    .unwrap_or(self.config.sdk_config.compile_api_version);
            discover_ark_vm_version(&self.config.sdk_config.ets_loader_path, profile_api_version)
        };
        let profile_processor = ProcessProfileProcessor::new();
        let mut manifest = {
            let _timer = ProfileTimingProbe::new(&module_cfg.name, "process_profile");
            if let Some(ark_vm_version) = discovered_ark_vm_version {
                profile_processor.process_profile_with_ark_vm_version(
                    module_json5,
                    &ark_vm_version,
                    &self.config.product.bundle_name,
                    CompileMode::EsModule,
                )
            } else {
                profile_processor.process_profile(
                    module_json5,
                    self.config.sdk_config.compile_api_version,
                    &self.config.product.bundle_name,
                    CompileMode::EsModule,
                )
            }
        };

        // `ProcessProfile` replaces source `module.dependencies` with the HSP
        // set returned by `mergeHspLibs()`. The dependency session is the
        // authoritative source for installed HSP module identity.
        manifest.dependencies = dependency_context.hsp_dependencies().to_vec();

        // Auto-derive HSP dependencies from workspace modules.
        //
        // hvigor's ProcessProfile.addModuleDependencies() automatically collects
        // Shared-type workspace modules (HSPs) and adds them to module.dependencies
        // so the HAP's module.json correctly lists all HSP dependencies.
        //
        // For workspace HSPs, the bundleName is the app's bundleName (they share
        // the same app bundle).  We add workspace HSPs not already in the list.
        if matches!(
            manifest.module_type,
            arkdown_core::OhosModuleType::Entry | arkdown_core::OhosModuleType::Feature
        ) {
            let existing_module_names: std::collections::HashSet<String> = manifest
                .dependencies
                .iter()
                .map(|d| d.module_name.clone())
                .collect();
            for workspace_module in &self.config.modules {
                if workspace_module.name == module_cfg.name {
                    continue; // skip self
                }
                if workspace_module.module_type != arkdown_core::OhosModuleType::Shared {
                    continue; // only SharedHsp
                }
                if existing_module_names.contains(&workspace_module.name) {
                    continue; // already declared
                }
                // Workspace SharedHsp: bundleName = app bundleName (same project)
                manifest.dependencies.push(arkdown_core::HspDependency {
                    module_name: workspace_module.name.clone(),
                    version_code: dependency_context.app_version_code,
                    bundle_name: None,
                });
            }
        }

        let loader = {
            let _timer = ProfileTimingProbe::new(&module_cfg.name, "loader_json_try_generate");
            LoaderJson::try_generate_with_dependency_graph(
                &self.config,
                module_cfg,
                &manifest,
                dependency_context.graph(),
            )?
        };
        Ok((manifest, loader))
    }

    /// Serialize `loader` as indented JSON and write it to `path`.
    ///
    /// Creates parent directories if they do not exist.
    pub fn write_loader_json(&self, loader: &LoaderJson, path: &Path) -> Result<(), ArkdownError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(loader)
            .map_err(|e| ArkdownError::Io(std::io::Error::other(e.to_string())))?;
        fs_err::write(path, json)?;

        Ok(())
    }

    /// Write the processed `module.json` for inclusion in the HAP.
    ///
    /// The output is a standard JSON file (not JSON5) combining:
    /// - `app` section from `AppScope/app.json5` (falls back to config values)
    /// - `module` section from the processed manifest (with `virtualMachine` and `compileMode`)
    ///
    /// If `resource_ids` is provided, resource references in `icon`/`label` (app section)
    /// and `description` (module section) are resolved to numeric IDs and written as
    /// `iconId`, `labelId`, and `descriptionId` respectively.
    pub fn write_module_json(
        &self,
        manifest: &HapManifest,
        output_path: &Path,
        resource_ids: Option<&FxHashMap<String, u32>>,
    ) -> Result<(), ArkdownError> {
        ModuleJsonWriter::write(manifest, &self.config, output_path, resource_ids)
    }

    pub fn write_module_json_with_dependency_context(
        &self,
        manifest: &HapManifest,
        output_path: &Path,
        resource_ids: Option<&FxHashMap<String, u32>>,
        dependency_context: &ProfileDependencyContext,
    ) -> Result<(), ArkdownError> {
        ModuleJsonWriter::write_with_dependency_graph(
            manifest,
            &self.config,
            output_path,
            resource_ids,
            dependency_context.graph(),
        )
    }

    /// Annotate an existing processed `module.json` with resource IDs from
    /// SDK `restool`, preserving profile-stage mutations already written to it.
    pub fn annotate_module_json_resource_ids(
        &self,
        output_path: &Path,
        resource_ids: &FxHashMap<String, u32>,
    ) -> Result<(), ArkdownError> {
        ModuleJsonWriter::annotate_resource_ids(output_path, resource_ids)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use arkdown_core::{
        AbilityConfig, ArkdownConfig, BuildMode, BuildOption, ModuleConfig, ModuleJson5,
        ModuleJson5Module, NativeLibOption, OhosModuleType, OutputTarget, ProductConfig, SdkConfig,
        StrictModeConfig,
    };

    use super::*;

    fn make_config() -> ArkdownConfig {
        ArkdownConfig {
            project_root: PathBuf::from("/project"),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.testapp".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                compatible_sdk_version_stage: None,
                target_sdk_version: None,
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: false,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                    preload_system_so: false,
                    preload_so: vec![],
                    deduplicate_har: false,
                    remove_permissions: vec![],
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: vec![],
                    external_native_options: None,
                    har_artifact_type: arkdown_core::HarArtifactType::Original,
                    native_lib: NativeLibOption::default(),
                },
                version_code: None,
                version_name: None,
            },
            modules: vec![],
            sdk_config: SdkConfig {
                ets_loader_path: PathBuf::from("/sdk/ets-loader"),
                es2abc_path: PathBuf::from("/sdk/es2abc"),
                restool_path: PathBuf::from("/sdk/restool"),
                compile_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        }
    }

    fn make_module_json5() -> ModuleJson5 {
        ModuleJson5 {
            module: ModuleJson5Module {
                name: "entry".to_string(),
                type_: "entry".to_string(),
                main_element: Some("EntryAbility".to_string()),
                device_types: vec!["phone".to_string()],
                pages: None,
                router_map: None,
                description: None,
                abilities: vec![AbilityConfig {
                    name: "EntryAbility".to_string(),
                    src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                    exported: true,
                    skills: vec![],
                    icon: None,
                    label: None,
                    description: None,
                    start_window_icon: None,
                    start_window_background: None,
                }],
                extension_abilities: vec![],
                src_entry: None,
                metadata: vec![],
                app_startup: None,
                cross_app_shared_config: Default::default(),
                ..Default::default()
            },
        }
    }

    fn make_module_cfg() -> ModuleConfig {
        ModuleConfig {
            name: "entry".to_string(),
            src_path: PathBuf::from("/project/entry"),
            module_type: OhosModuleType::Entry,
        }
    }

    #[cfg(unix)]
    fn write_fake_es2abc(es2abc: &Path) {
        fs::create_dir_all(es2abc.parent().expect("es2abc parent"))
            .expect("create fake es2abc dir");
        fs::write(
            es2abc,
            r#"#!/bin/sh
stage=""
previous=""
for arg in "$@"; do
  if [ "$previous" = "--target-api-sub-version" ]; then
    stage="$arg"
  fi
  previous="$arg"
done
if [ "$stage" = "Release" ]; then
  printf '12.0.6.0\n'
else
  printf '12.0.2.0\n'
fi
"#,
        )
        .expect("write fake es2abc");
        let mut permissions = fs::metadata(es2abc)
            .expect("fake es2abc metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(es2abc, permissions).expect("chmod fake es2abc");
    }

    #[test]
    fn test_profile_processor_process_module() {
        let config = Arc::new(make_config());
        let processor = ProfileProcessor::new(config);
        let module_cfg = make_module_cfg();
        let module_json5 = make_module_json5();

        let (manifest, loader) = processor
            .process_module(&module_cfg, &module_json5)
            .expect("process_module failed");

        // virtualMachine from api version 12 → "ark12.0.1.0"
        assert_eq!(manifest.virtual_machine, "ark12.0.1.0");
        assert_eq!(manifest.module_name, "entry");

        // loader.json should contain one compile entry
        assert_eq!(loader.compile_entry.len(), 1);
        assert_eq!(
            loader.compile_entry[0],
            "./ets/entryability/EntryAbility.ets"
        );
        assert_eq!(loader.build_mode, "debug");
    }

    #[test]
    fn test_process_module_does_not_advertise_dependency_only_router_map() {
        let temp_dir = tempfile::tempdir().expect("create temp project");
        let project_root = temp_dir.path().to_path_buf();
        let entry_root = project_root.join("entry");
        let feature_root = project_root.join("feature");
        let feature_profile_dir = feature_root.join("src/main/resources/base/profile");
        fs::create_dir_all(&feature_profile_dir).expect("create feature profile dir");
        fs::create_dir_all(feature_root.join("src/main")).expect("create feature module dir");
        fs::write(
            feature_root.join("src/main/module.json5"),
            r#"{"module":{"name":"feature","type":"har","routerMap":"$profile:router_map"}}"#,
        )
        .expect("write feature module.json5");
        fs::write(
            feature_profile_dir.join("router_map.json"),
            r#"{"routerMap":[{"name":"detail","pageSourceFile":"src/main/ets/pages/Detail.ets","buildFunction":"DetailBuilder"}]}"#,
        )
        .expect("write feature router map");

        let mut config = make_config();
        config.project_root = project_root;
        config.modules = vec![
            ModuleConfig {
                name: "entry".to_string(),
                src_path: entry_root.clone(),
                module_type: OhosModuleType::Entry,
            },
            ModuleConfig {
                name: "feature".to_string(),
                src_path: feature_root,
                module_type: OhosModuleType::Har,
            },
        ];
        let processor = ProfileProcessor::new(Arc::new(config));
        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: entry_root,
            module_type: OhosModuleType::Entry,
        };

        let (manifest, loader) = processor
            .process_module(&module_cfg, &make_module_json5())
            .expect("process_module failed");

        assert!(!loader.router_map.is_empty());
        assert_eq!(manifest.router_map, None);
    }

    #[test]
    fn test_router_map_uses_host_dependency_order() {
        let temp_dir = tempfile::tempdir().expect("create temp project");
        let project_root = temp_dir.path().to_path_buf();
        let entry_root = project_root.join("entry");
        fs::create_dir_all(&entry_root).expect("create entry root");
        fs::write(
            entry_root.join("oh-package.json5"),
            r#"{"name":"entry","dependencies":{"@ohos/beta":"file:../beta","@ohos/alpha":"file:../alpha"}}"#,
        )
        .expect("write entry package");

        for module_name in ["alpha", "beta"] {
            let module_root = project_root.join(module_name);
            let profile_dir = module_root.join("src/main/resources/base/profile");
            fs::create_dir_all(&profile_dir).expect("create profile dir");
            fs::write(
                module_root.join("src/main/module.json5"),
                format!(
                    r#"{{"module":{{"name":"{module_name}","type":"har","routerMap":"$profile:router_map"}}}}"#
                ),
            )
            .expect("write module.json5");
            fs::write(
                profile_dir.join("router_map.json"),
                format!(
                    r#"{{"routerMap":[{{"name":"{module_name}","pageSourceFile":"src/main/ets/pages/{module_name}.ets","buildFunction":"{module_name}Builder"}}]}}"#
                ),
            )
            .expect("write router map");
        }

        let mut config = make_config();
        config.project_root = project_root.clone();
        config.modules = vec![
            ModuleConfig {
                name: "entry".to_string(),
                src_path: entry_root.clone(),
                module_type: OhosModuleType::Entry,
            },
            ModuleConfig {
                name: "alpha".to_string(),
                src_path: project_root.join("alpha"),
                module_type: OhosModuleType::Har,
            },
            ModuleConfig {
                name: "beta".to_string(),
                src_path: project_root.join("beta"),
                module_type: OhosModuleType::Har,
            },
        ];
        let processor = ProfileProcessor::new(Arc::new(config));
        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: entry_root,
            module_type: OhosModuleType::Entry,
        };

        let (_, loader) = processor
            .process_module(&module_cfg, &make_module_json5())
            .expect("process_module failed");
        let names = loader
            .router_map
            .iter()
            .filter_map(|entry| entry.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["beta", "alpha"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_profile_processor_uses_discovered_ark_vm_version() {
        let temp_dir = tempfile::tempdir().expect("create temp sdk");
        let ets_dir = temp_dir.path().join("ets");
        let ets_loader = ets_dir.join("build-tools").join("ets-loader");
        let es2abc = ets_loader
            .join("bin")
            .join("ark")
            .join(if cfg!(target_os = "macos") {
                "build-mac"
            } else if cfg!(target_os = "windows") {
                "build-win"
            } else {
                "build-linux"
            })
            .join("bin")
            .join("es2abc");

        fs::create_dir_all(&ets_dir).expect("create fake ets dir");
        fs::write(
            ets_dir.join("oh-uni-package.json"),
            r#"{"version":"6.0.1.112","releaseType":"Release"}"#,
        )
        .expect("write fake ets metadata");
        write_fake_es2abc(&es2abc);

        let mut config = make_config();
        config.sdk_config.ets_loader_path = ets_loader;
        config.sdk_config.es2abc_path = es2abc;
        let processor = ProfileProcessor::new(Arc::new(config));
        let module_cfg = make_module_cfg();
        let module_json5 = make_module_json5();

        let (manifest, _) = processor
            .process_module(&module_cfg, &module_json5)
            .expect("process_module failed");

        assert_eq!(manifest.virtual_machine, "ark12.0.6.0");
    }

    /// Verify that `write_module_json` produces output that matches the golden
    /// `tests/fixtures/golden/minimal-hap/module.json` exactly (both field
    /// values and insertion order).
    #[test]
    fn test_write_module_json_golden_parity() {
        use arkdown_core::{AbilityConfig, ModuleJson5Module, SkillConfig};

        // Locate the repo root from the crate manifest dir.
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent() // crates/arkdown-profile
            .unwrap()
            .parent() // crates/
            .unwrap()
            .parent() // repo root
            .unwrap()
            .to_path_buf();

        let golden_path = repo_root.join("tests/fixtures/golden/minimal-hap/module.json");
        if !golden_path.exists() {
            // Skip when the golden file is absent (e.g. in a stripped checkout).
            tracing::error!("Skipping golden test: {:?} not found", golden_path);
            return;
        }

        // Config that matches the minimal-hap fixture.
        let fixture = repo_root.join("tests/fixtures/minimal-hap");
        let mut config = make_config();
        config.project_root = fixture.clone();
        config.product.bundle_name = "com.example.arkdown.test".to_string();
        config.product.compatible_sdk_version = "5.0.5(17)".to_string(); // → 50005017
        config.sdk_config.compile_sdk_version = "6.0.1.112".to_string();
        config.build_mode = BuildMode::Debug;

        let processor = ProfileProcessor::new(Arc::new(config));

        // Construct the manifest matching module.json5.
        let module_json5 = ModuleJson5 {
            module: ModuleJson5Module {
                name: "entry".to_string(),
                type_: "entry".to_string(),
                main_element: Some("EntryAbility".to_string()),
                device_types: vec!["phone".to_string()],
                pages: Some("$profile:main_pages".to_string()),
                router_map: None,
                description: Some("$string:module_desc".to_string()),
                abilities: vec![AbilityConfig {
                    name: "EntryAbility".to_string(),
                    src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                    exported: true,
                    skills: vec![SkillConfig {
                        entities: vec!["entity.system.home".to_string()],
                        actions: vec!["ohos.want.action.home".to_string()],
                        uris: vec![],
                    }],
                    icon: Some("$media:layered_image".to_string()),
                    label: Some("$string:EntryAbility_label".to_string()),
                    description: Some("$string:EntryAbility_desc".to_string()),
                    start_window_icon: Some("$media:startIcon".to_string()),
                    start_window_background: Some("$color:start_window_background".to_string()),
                }],
                extension_abilities: vec![],
                src_entry: None,
                metadata: vec![],
                app_startup: None,
                cross_app_shared_config: Default::default(),
                ..Default::default()
            },
        };

        let module_cfg = ModuleConfig {
            name: "entry".to_string(),
            src_path: fixture.join("entry"),
            module_type: OhosModuleType::Entry,
        };

        let manifest = ProcessProfileProcessor::new().process_profile_with_ark_vm_version(
            &module_json5,
            "12.0.6.0",
            "com.example.arkdown.test",
            arkdown_core::CompileMode::EsModule,
        );
        let _ = module_cfg; // silence unused warning

        // Resource IDs from the golden.
        let mut resource_ids = FxHashMap::default();
        resource_ids.insert("layered_image".to_string(), 16777219u32);
        resource_ids.insert("app_name".to_string(), 16777216u32);
        resource_ids.insert("EntryAbility_desc".to_string(), 16777220u32);
        resource_ids.insert("EntryAbility_label".to_string(), 16777221u32);
        resource_ids.insert("start_window_background".to_string(), 16777223u32);
        resource_ids.insert("startIcon".to_string(), 16777224u32);
        resource_ids.insert("module_desc".to_string(), 16777222u32);

        let tmp_dir = tempfile::tempdir().expect("create temp dir");
        let out_path = tmp_dir.path().join("module.json");

        processor
            .write_module_json(&manifest, &out_path, Some(&resource_ids))
            .expect("write_module_json failed");

        let produced_raw = fs::read_to_string(&out_path).expect("read produced module.json");
        let golden_raw = fs::read_to_string(&golden_path).expect("read golden module.json");

        // Compare as JSON values first (content check).
        let produced: serde_json::Value =
            serde_json::from_str(&produced_raw).expect("parse produced module.json");
        let golden: serde_json::Value =
            serde_json::from_str(&golden_raw).expect("parse golden module.json");
        assert_eq!(
            produced, golden,
            "module.json content mismatch vs golden.\nProduced:\n{}\nGolden:\n{}",
            produced_raw, golden_raw
        );

        // Compare key order in app section.
        let produced_app_keys: Vec<&str> = produced["app"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        let golden_app_keys: Vec<&str> = golden["app"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            produced_app_keys, golden_app_keys,
            "app field order mismatch vs golden"
        );

        // Compare key order in module section.
        let produced_module_keys: Vec<&str> = produced["module"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        let golden_module_keys: Vec<&str> = golden["module"]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            produced_module_keys, golden_module_keys,
            "module field order mismatch vs golden"
        );

        // Compare key order in abilities[0].
        let produced_ability_keys: Vec<&str> = produced["module"]["abilities"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        let golden_ability_keys: Vec<&str> = golden["module"]["abilities"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            produced_ability_keys, golden_ability_keys,
            "abilities[0] field order mismatch vs golden"
        );
    }

    #[test]
    fn test_write_loader_json_creates_file() {
        let config = Arc::new(make_config());
        let processor = ProfileProcessor::new(config);
        let module_cfg = make_module_cfg();
        let module_json5 = make_module_json5();

        let (_, loader) = processor
            .process_module(&module_cfg, &module_json5)
            .expect("process_module failed");

        let tmp_dir = std::env::temp_dir().join("arkdown_profile_test");
        let out_path = tmp_dir.join("loader.json");

        processor
            .write_loader_json(&loader, &out_path)
            .expect("write_loader_json failed");

        assert!(out_path.exists(), "loader.json was not created");

        // Read back and deserialize
        let content = std::fs::read_to_string(&out_path).unwrap();
        let loaded: LoaderJson =
            serde_json::from_str(&content).expect("failed to parse written loader.json");
        assert_eq!(loaded.build_mode, "debug");
        assert_eq!(loaded.package_manager_type, "ohpm");

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    /// Verify that workspace SharedHsp modules are automatically added to
    /// manifest.dependencies in process_module().
    ///
    /// This mirrors hvigor's ProcessProfile.addModuleDependencies() which
    /// collects all Shared-type workspace modules and adds them to the HAP's
    /// module.dependencies so the final module.json lists HSP dependencies.
    ///
    /// Addresses section 3.2: '由依赖图自动派生的 HSP metadata、transitive/order'
    #[test]
    fn test_process_module_auto_derives_workspace_hsp_dependencies() {
        let mut config = make_config();
        // Add a SharedHsp workspace module alongside the entry module.
        config.modules = vec![
            ModuleConfig {
                name: "entry".to_string(),
                src_path: PathBuf::from("/project/entry"),
                module_type: OhosModuleType::Entry,
            },
            ModuleConfig {
                name: "mySharedHsp".to_string(),
                src_path: PathBuf::from("/project/mySharedHsp"),
                module_type: OhosModuleType::Shared,
            },
        ];

        let processor = ProfileProcessor::new(Arc::new(config.clone()));
        let module_cfg = make_module_cfg(); // entry module
        let module_json5 = make_module_json5(); // no explicit dependencies

        let (manifest, _) = processor
            .process_module(&module_cfg, &module_json5)
            .expect("process_module failed");

        // The SharedHsp workspace module must be auto-derived into dependencies.
        assert!(
            manifest
                .dependencies
                .iter()
                .any(|d| d.module_name == "mySharedHsp"),
            "workspace SharedHsp must be auto-added to manifest.dependencies; got: {:?}",
            manifest.dependencies
        );
        // bundleName should be the app's bundleName.
        let hsp_dep = manifest
            .dependencies
            .iter()
            .find(|d| d.module_name == "mySharedHsp")
            .unwrap();
        assert_eq!(hsp_dep.bundle_name, None);
    }
}
