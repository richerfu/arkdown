use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::{ArkdownConfig, HarArtifactType, RuntimeOnlyConfig};
use arkdown_path::{AbsPath, ArtifactPath, ArtifactPathSerializer};
use indexmap::IndexMap;

use crate::error::PackError;
use crate::integrated_hsp::IntegratedHspEntry;
use crate::zip_writer;

pub fn minify_package_intermediate_module_json(content: &str) -> Result<String, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(content)?;
    serde_json::to_string(&order_package_module_json(value))
}

fn order_package_module_json(value: serde_json::Value) -> serde_json::Value {
    let Some(root) = value.as_object() else {
        return value;
    };

    let mut out = serde_json::Map::new();
    if let Some(app) = root.get("app") {
        out.insert(
            "app".to_string(),
            order_object(
                app,
                &[
                    "bundleName",
                    "vendor",
                    "versionCode",
                    "versionName",
                    "icon",
                    "label",
                    "apiReleaseType",
                    "compileSdkVersion",
                    "targetAPIVersion",
                    "minAPIVersion",
                    "compileSdkType",
                    "targetMinorAPIVersion",
                    "targetPatchAPIVersion",
                    "appEnvironments",
                    "bundleType",
                    "buildMode",
                    "debug",
                    "iconId",
                    "labelId",
                ],
            ),
        );
    }
    if let Some(module) = root.get("module") {
        out.insert("module".to_string(), order_module(module));
    }
    for (key, value) in root {
        if !out.contains_key(key) {
            out.insert(key.clone(), value.clone());
        }
    }
    serde_json::Value::Object(out)
}

fn order_module(module: &serde_json::Value) -> serde_json::Value {
    let Some(object) = module.as_object() else {
        return module.clone();
    };
    let mut out = serde_json::Map::new();
    for key in [
        "name",
        "type",
        "srcEntry",
        "description",
        "mainElement",
        "deviceTypes",
        "deliveryWithInstall",
        "installationFree",
        "pages",
        "routerMap",
    ] {
        if let Some(value) = object.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    if let Some(abilities) = object.get("abilities") {
        out.insert("abilities".to_string(), order_abilities(abilities));
    }
    for key in [
        "packageName",
        "virtualMachine",
        "compileMode",
        // `formExtensionModule` sits between `type` and `dependencies` in the
        // SDK packaging order (observed on a form widget HSP archive).
        "formExtensionModule",
        "dependencies",
        "descriptionId",
    ] {
        if let Some(value) = object.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    for (key, value) in object {
        if !out.contains_key(key) {
            out.insert(key.clone(), value.clone());
        }
    }
    serde_json::Value::Object(out)
}

fn order_abilities(abilities: &serde_json::Value) -> serde_json::Value {
    let Some(array) = abilities.as_array() else {
        return abilities.clone();
    };
    serde_json::Value::Array(
        array
            .iter()
            .map(|ability| {
                let ordered = order_object(
                    ability,
                    &[
                        "name",
                        "srcEntry",
                        "description",
                        "icon",
                        "label",
                        "exported",
                        "startWindowIcon",
                        "startWindowBackground",
                        "skills",
                        "descriptionId",
                        "iconId",
                        "labelId",
                        "startWindowIconId",
                        "startWindowBackgroundId",
                    ],
                );
                let serde_json::Value::Object(mut object) = ordered else {
                    return ordered;
                };
                if let Some(skills) = object.get("skills").cloned() {
                    object.insert("skills".to_string(), order_skills(&skills));
                }
                serde_json::Value::Object(object)
            })
            .collect(),
    )
}

fn order_skills(skills: &serde_json::Value) -> serde_json::Value {
    let Some(array) = skills.as_array() else {
        return skills.clone();
    };
    serde_json::Value::Array(
        array
            .iter()
            .map(|skill| order_object(skill, &["entities", "actions", "uris"]))
            .collect(),
    )
}

fn order_object(value: &serde_json::Value, fields: &[&str]) -> serde_json::Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut out = serde_json::Map::new();
    for key in fields {
        if let Some(value) = object.get(*key) {
            out.insert((*key).to_string(), value.clone());
        }
    }
    for (key, value) in object {
        if !out.contains_key(key) {
            out.insert(key.clone(), value.clone());
        }
    }
    serde_json::Value::Object(out)
}

pub struct Packager {
    pub config: Arc<ArkdownConfig>,
    compress_level: u32,
}

/// A single ability entry included in `pack.info` under `modules[].abilities`.
pub struct PackAbility {
    pub name: String,
    pub label: Option<String>,
}

pub struct PackInput {
    pub module_name: String,
    /// Path to `modules.abc` produced by SDK es2abc.
    pub abc_path: PathBuf,
    /// Path to `resources.index` produced by arkdown-resource.
    pub resources_index: PathBuf,
    /// Path to the processed `module.json` produced by arkdown-profile.
    pub module_json: PathBuf,
    /// Current-module profile used by Hvigor's MakePackInfo task.
    ///
    /// This is deliberately separate from `module_json`: ProcessProfile may
    /// append dependency HAR abilities before packaging, while MakePackInfo
    /// reads the current module model and must not report dependency entries.
    pub pack_info_module_json: Option<PathBuf>,
    /// Optional native `.so` libraries to bundle.
    pub native_libs: Vec<PathBuf>,
    /// Whether native `.so` libraries should be deflated in the package.
    pub compress_native_libs: bool,
    /// Destination archive path, e.g. `build/.../entry.hap`.
    pub output_path: PathBuf,
    /// Directory whose contents are copied into HAP under `resources/`.
    ///
    /// The CLI passes the resource compiler's processed `.../res/default/resources`
    /// directory so the archive contains restool-style resource files rather than
    /// raw source `element/*.json` inputs.
    pub resources_src_dir: Option<PathBuf>,
    /// Bundle version code (from `AppScope/app.json5` `versionCode`).
    pub bundle_version_code: u64,
    /// Bundle version name (from `AppScope/app.json5` `versionName`).
    pub bundle_version_name: String,
    /// Optional effective build version.
    ///
    /// Hvigor emits this as `summary.app.version.build` after `code` and
    /// `name`, and omits it entirely when no buildVersion is configured.
    pub bundle_build_version: Option<String>,
    /// Bundle type: `"app"` for HAP, `"shared"` for HSP.
    pub bundle_type: String,
    /// Path to `sourceMaps.map` produced by the bundler.
    pub source_maps_path: Option<PathBuf>,
    /// Path to `pkgContextInfo.json` produced by GeneratePkgContextInfo.
    ///
    /// Hvigor packages this at the HAP root when normalized OHM URLs are enabled.
    /// ArkRuntime uses it to map esmodule entry paths onto records inside
    /// `ets/modules.abc`.
    pub pkg_context_info_path: Option<PathBuf>,
    /// Path to `pkgSdkInfo.json` produced by GeneratePkgSdkInfo.
    ///
    /// Hvigor packages this at the HAP root when normalized OHM URLs are
    /// disabled. It is mutually exclusive with `pkg_context_info_path`.
    pub pkg_sdk_info_path: Option<PathBuf>,
    /// Abilities list for inclusion in `pack.info` under `modules[].abilities`.
    pub abilities: Vec<PackAbility>,
    /// Remote HSPs to embed inside this HAP (integrated HSP feature, API >= 11).
    ///
    /// Each entry is stored at `libs/<hsp_dir_name>/<hsp_file_name>` inside the archive.
    /// Pass an empty vec for HAPs without integrated HSPs (the common case).
    pub integrated_hsps: Vec<IntegratedHspEntry>,
    pub privacy_profile_path: Option<std::path::PathBuf>,
    /// `rpcid.sc` emitted by the SDK `syscap_tool` for OpenHarmony targets.
    /// HarmonyOS targets and HAR modules do not provide this input.
    pub syscap_path: Option<PathBuf>,
    /// Device types from the module manifest (e.g. `["phone", "tablet"]`).
    /// Used in pack.info `deviceType` field.  Defaults to `["phone"]` when empty.
    pub device_types: Vec<String>,
    /// Whether this module is installation-free (atomicService bundles).
    /// Used in pack.info `distro.installationFree` field.
    pub installation_free: bool,
    /// Whether this module is delivered with install.
    /// Used in pack.info `distro.deliveryWithInstall` and `packages[].deliveryWithInstall`.
    pub delivery_with_install: bool,
}

pub struct HarPackInput {
    pub module_name: String,
    /// Source ETS files directory (`src/main/ets/`).
    pub source_dir: PathBuf,
    /// Resources directory (`src/main/resources/`).
    pub resources_dir: PathBuf,
    /// Permission reason resources generated by MergeProfile as `resource_str`.
    /// Packaged under `src/main/resource/` for source HAR/interface HAR layout.
    pub resource_str_dir: Option<PathBuf>,
    /// Path to `oh-package.json5`.
    pub oh_package_json: PathBuf,
    /// Destination archive path.
    pub output_path: PathBuf,
    /// Directory containing declaration stubs (`index.d.ts`, `index.d.ets`).
    ///
    /// Hvigor copies these intermediates for compiled HAR variants. Original
    /// source HARs retain their ArkTS sources and deliberately omit generated
    /// declarations. When `None` (or the directory does not exist), declaration
    /// stubs are omitted.
    pub declarations_dir: Option<PathBuf>,
    /// Path to the consumer obfuscation rules file (`obfuscation.txt`).
    ///
    /// hvigor calls `generateConsumerObConfigFile()` and includes the result so
    /// consumers know which symbols the HAR exposes and how to configure their
    /// own obfuscation for packages that depend on this HAR.
    /// When `None` (or the file does not exist), consumer rules are omitted.
    pub consumer_rules_path: Option<PathBuf>,
}

/// Variant-specific HAR intermediates produced by the compile, profile,
/// resource, and native stages. `AbstractProcessHarArtifacts` assembles these
/// into one temporary directory before calling ohpm; arkdown models the same
/// inputs explicitly so no source manifest has to be mutated.
#[derive(Debug, Clone, Default)]
pub struct HarPackageParameterInput {
    pub version: Option<String>,
    pub dependencies: IndexMap<String, String>,
    pub dev_dependencies: IndexMap<String, String>,
    pub dynamic_dependencies: IndexMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct HarBundledDependencyInput {
    pub dependencies: IndexMap<String, String>,
    pub dynamic_dependencies: IndexMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct HarVariantInput {
    pub artifact_type: HarArtifactType,
    pub module_root: PathBuf,
    /// `intermediates/loader_out/<target>`; obfuscated HAR copies the complete
    /// tree while ByteCode HAR copies only its `ets/` child.
    pub compiled_output_dir: PathBuf,
    pub processed_module_json: Option<PathBuf>,
    pub resource_table_path: Option<PathBuf>,
    pub native_libs: Vec<PathBuf>,
    pub native_header_dirs: Vec<PathBuf>,
    pub preload_system_so_json: Option<PathBuf>,
    pub source_maps_path: Option<PathBuf>,
    /// `HarBuildProfileTask` output. When supplied, this is the authoritative
    /// generated profile and may contain target `buildProfileFields`.
    pub build_profile_path: Option<PathBuf>,
    pub declaration_entries: Vec<String>,
    /// Values resolved from the project `parameterFile` for hvigor's
    /// `version`/dependency whitelist. The manifest writer replaces these in
    /// place so source key order remains unchanged.
    pub package_parameters: HarPackageParameterInput,
    /// Effective `buildOption.sourceOption.workers` entries for
    /// `ProcessOhPackageJson.addWorkerData`.
    pub worker_entries: Vec<PathBuf>,
    /// Effective `buildOption.arkOptions.runtimeOnly` value for
    /// `ProcessOhPackageJson.addDynamicImport`.
    pub runtime_only: Option<RuntimeOnlyConfig>,
    /// Effective target resource directories serialized by
    /// `ProcessOhPackageJson.processResourceConfig`.
    pub resource_directories: Vec<PathBuf>,
    /// `ProcessHarArtifacts.deleteOhPkgDep` result. `Some` means the runtime
    /// observed a falsy `hvigor.keepDependency` for a bundled ByteCode HAR.
    pub bundled_dependency_rewrite: Option<HarBundledDependencyInput>,
    /// Direct/all dependency versions selected by
    /// `ProcessHarArtifacts.setDependencyPkgVersion`.
    pub dependency_pkg_version: IndexMap<String, String>,
    pub bundled_dependencies: bool,
    pub bundled_resources: Vec<BundledHarResourceInput>,
    pub bundled_resource_directories: Vec<ArtifactPath>,
    pub compiled_package_names: Vec<String>,
    pub extra_importees: Vec<String>,
    pub global_dependencies: Vec<String>,
    pub ignore_resource_patterns: Vec<String>,
    pub debug: bool,
    /// Effective module target selected by hvigor's target service.
    /// Used by `HarBuildProfileTask` for `TARGET_NAME`.
    pub target_name: String,
    /// Latest hvigor materializes the complete `PackageHar` input tree below
    /// `build/<product>/cache/<target>/<target>@PackageHar` before invoking
    /// ohpm. Keeping this optional lets library callers omit build-cache state.
    pub package_cache_dir: Option<PathBuf>,
}

/// One dependency resource directory staged by latest hvigor's
/// `ProcessHarArtifacts.copyBundledDepRes`.
#[derive(Debug, Clone)]
pub struct BundledHarResourceInput {
    pub source_dir: PathBuf,
    pub archive_directory: ArtifactPath,
}

/// Inputs consumed by hvigor's `PrepareSharedHarRes` and `PackageSharedHar`
/// tasks. The resulting interface HAR is emitted beside the HSP and exposes
/// declarations, resources, and native link inputs to consumers without
/// shipping the HSP implementation sources.
#[derive(Debug, Clone)]
pub struct HspInterfaceHarInput {
    pub module_name: String,
    pub module_root: PathBuf,
    pub target_name: String,
    pub oh_package_json: PathBuf,
    pub processed_module_json: PathBuf,
    pub declarations_dir: PathBuf,
    pub resource_table_path: Option<PathBuf>,
    pub resource_str_dir: Option<PathBuf>,
    pub privacy_profile_path: Option<PathBuf>,
    pub obfuscation_rules_path: Option<PathBuf>,
    pub native_libs: Vec<PathBuf>,
    pub native_header_dirs: Vec<PathBuf>,
    pub exclude_so_from_interface_har: bool,
    pub output_path: PathBuf,
    pub compatible_sdk_version: u32,
    pub compatible_sdk_type: String,
    pub use_normalized_ohm_url: bool,
    pub integrated_hsp: Option<bool>,
    pub source_roots: Vec<String>,
    pub dependencies: BTreeMap<String, String>,
    pub dynamic_dependencies: BTreeMap<String, String>,
    pub debug: bool,
    pub obfuscated: bool,
}

/// Inputs consumed by hvigor's `PackageSharedTgz` task.
///
/// The archive contains exactly the distributable HSP (renamed without the
/// `-unsigned` marker) and the interface HAR at its root.
#[derive(Debug, Clone)]
pub struct HspTgzInput {
    pub hsp_path: PathBuf,
    pub interface_har_path: PathBuf,
    pub output_path: PathBuf,
}

impl Packager {
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        Packager {
            config,
            compress_level: 1,
        }
    }

    /// Select the SDK deflate level for compressed native libraries.
    /// Latest hvigor maps `fast`, `standard`, and `ultimate` to 1, 5, and 9.
    pub fn with_compress_level(mut self, compress_level: u32) -> Self {
        self.compress_level = compress_level;
        self
    }

    /// Package a HAP archive.
    ///
    /// Layout:
    /// ```text
    /// module.json           (stored)
    /// resources.index       (stored)
    /// ets/modules.abc       (stored, API >= 9)  or  modules.abc  (stored, API < 9)
    /// libs/arm64-v8a/*.so   (stored, optional)
    /// ```
    fn generate_pack_info(
        &self,
        input: &PackInput,
        module_json: Option<&serde_json::Value>,
    ) -> String {
        let abilities_json: Vec<serde_json::Value> = input
            .abilities
            .iter()
            .map(|a| {
                let mut obj = serde_json::json!({"name": a.name});
                if let Some(label) = &a.label {
                    obj["label"] = serde_json::json!(label);
                }
                obj
            })
            .collect();

        let extension_abilities = Self::pack_info_extension_abilities(input, module_json);
        let compatible_api_version =
            compatible_api_version(&self.config.product.compatible_sdk_version)
                .unwrap_or(self.config.sdk_config.compile_api_version);

        // MakePackInfo keeps the application bundle type independent from the
        // module type. In particular an HSP normally belongs to an `app`
        // bundle while its module/package type is `shared`.
        let module_type = module_json
            .and_then(|value| value.pointer("/module/type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("entry");

        let device_types = if input.device_types.is_empty() {
            vec!["phone".to_string()]
        } else {
            input.device_types.clone()
        };

        let mut module = serde_json::Map::new();
        if let Some(main_ability) = module_json
            .and_then(|value| value.pointer("/module/mainElement"))
            .and_then(serde_json::Value::as_str)
        {
            module.insert("mainAbility".to_string(), main_ability.into());
        }
        module.insert(
            "deviceType".to_string(),
            serde_json::json!(device_types.clone()),
        );
        if !abilities_json.is_empty() {
            module.insert(
                "abilities".to_string(),
                serde_json::Value::Array(abilities_json),
            );
        }
        if !extension_abilities.is_empty() {
            module.insert(
                "extensionAbilities".to_string(),
                serde_json::Value::Array(extension_abilities),
            );
        }
        module.insert(
            "distro".to_string(),
            serde_json::json!({
                "moduleType": module_type,
                "installationFree": input.installation_free,
                "deliveryWithInstall": input.delivery_with_install,
                "moduleName": input.module_name
            }),
        );
        module.insert(
            "apiVersion".to_string(),
            serde_json::json!({
                "compatible": compatible_api_version,
                "releaseType": "Release",
                "target": self.config.sdk_config.compile_api_version
            }),
        );

        let mut version = serde_json::Map::new();
        version.insert("code".to_string(), input.bundle_version_code.into());
        version.insert("name".to_string(), input.bundle_version_name.clone().into());
        if let Some(build_version) = input.bundle_build_version.as_ref() {
            version.insert("build".to_string(), build_version.clone().into());
        }

        serde_json::json!({
            "summary": {
                "app": {
                    "bundleName": self.config.product.bundle_name,
                    "bundleType": input.bundle_type,
                    "version": serde_json::Value::Object(version)
                },
                "modules": [serde_json::Value::Object(module)]
            },
            "packages": [
                {
                    "deviceType": device_types,
                    "moduleType": module_type,
                    "deliveryWithInstall": input.delivery_with_install,
                    "name": format!("{}-default", input.module_name)
                }
            ]
        })
        .to_string()
    }

    fn pack_info_extension_abilities(
        input: &PackInput,
        module_json: Option<&serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        let Some(extension_abilities) = module_json
            .and_then(|value| value.pointer("/module/extensionAbilities"))
            .and_then(|value| value.as_array())
        else {
            return Vec::new();
        };

        let mut out = Vec::new();
        for ability in extension_abilities {
            let Some(ability_object) = ability.as_object() else {
                continue;
            };
            let Some(name) = ability_object.get("name").and_then(|value| value.as_str()) else {
                continue;
            };

            let mut entry = serde_json::Map::new();
            entry.insert("name".to_string(), name.into());
            if ability_object.contains_key("metadata") {
                let forms = input
                    .resources_src_dir
                    .as_deref()
                    .map(|resources_src_dir| {
                        Self::pack_info_forms(resources_src_dir, ability_object)
                    })
                    .unwrap_or_default();
                entry.insert("forms".to_string(), serde_json::Value::Array(forms));
            }
            out.push(serde_json::Value::Object(entry));
        }
        out
    }

    fn pack_info_forms(
        resources_src_dir: &std::path::Path,
        ability: &serde_json::Map<String, serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        let profile_names = ability
            .get("metadata")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter(|metadata| {
                metadata
                    .get("name")
                    .and_then(|value| value.as_str())
                    .is_some_and(|name| name.contains("form"))
            })
            .filter_map(|metadata| {
                metadata
                    .get("resource")
                    .and_then(|value| value.as_str())
                    .and_then(|resource| resource.strip_prefix("$profile:"))
            });

        let mut forms = Vec::new();
        for profile_name in profile_names {
            let profile_file = ProfileResourceFileName::new(profile_name);
            let profile_path = resources_src_dir
                .join("base")
                .join("profile")
                .join(profile_file.file_name());
            let Ok(content) = fs_err::read_to_string(&profile_path) else {
                continue;
            };
            let Ok(profile) = json5::from_str::<serde_json::Value>(&content) else {
                continue;
            };
            let Some(profile_forms) = profile.get("forms").and_then(|value| value.as_array())
            else {
                continue;
            };
            forms.extend(profile_forms.iter().filter_map(Self::pack_info_form_entry));
        }
        forms
    }

    fn pack_info_form_entry(form: &serde_json::Value) -> Option<serde_json::Value> {
        let object = form.as_object()?;
        let mut out = serde_json::Map::new();
        for key in [
            "name",
            "type",
            "updateEnabled",
            "scheduledUpdateTime",
            "updateDuration",
            "supportDimensions",
            "defaultDimension",
        ] {
            if key == "type" {
                let value = object.get(key).cloned().unwrap_or_else(|| "JS".into());
                out.insert(key.to_string(), value);
            } else if let Some(value) = object.get(key) {
                out.insert(key.to_string(), value.clone());
            }
        }
        (!out.is_empty()).then_some(serde_json::Value::Object(out))
    }

    pub fn pack_hap(&self, input: PackInput) -> Result<PathBuf, PackError> {
        if let Some(parent) = input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut writer = zip_writer::create_zip(&input.output_path)?;

        // module.json — minified and stored, matching hvigor PackageHap output.
        let module_json_value;
        if input.module_json.exists() {
            let module_json = fs_err::read_to_string(&input.module_json)?;
            let parsed_module_json = serde_json::from_str::<serde_json::Value>(&module_json).ok();
            let module_json_entry =
                PackageToolJson::from_bytes(module_json.as_bytes()).into_bytes();
            module_json_value = parsed_module_json;
            zip_writer::add_bytes_stored(&mut writer, "module.json", &module_json_entry)?;
        } else {
            return Err(PackError::MissingInput("module.json".to_string()));
        }
        let pack_info_module_json_value = if let Some(path) = &input.pack_info_module_json {
            if !path.is_file() {
                return Err(PackError::MissingInput(format!(
                    "pack.info module.json ({})",
                    path.display()
                )));
            }
            let content = fs_err::read_to_string(path)?;
            Some(
                serde_json::from_str::<serde_json::Value>(&content)
                    .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?,
            )
        } else {
            module_json_value.clone()
        };

        // resources.index — stored (binary resource table)
        if input.resources_index.exists() {
            zip_writer::add_file_stored(&mut writer, "resources.index", &input.resources_index)?;
        } else {
            return Err(PackError::MissingInput("resources.index".to_string()));
        }

        let pkg_context = if let Some(pkg_context_info_path) = &input.pkg_context_info_path {
            if !pkg_context_info_path.exists() {
                return Err(PackError::MissingInput(format!(
                    "pkgContextInfo.json ({})",
                    pkg_context_info_path.display()
                )));
            }
            Some(PackageToolJson::read(pkg_context_info_path)?.into_bytes())
        } else {
            None
        };
        let pkg_sdk = if let Some(pkg_sdk_info_path) = &input.pkg_sdk_info_path {
            if !pkg_sdk_info_path.exists() {
                return Err(PackError::MissingInput(format!(
                    "pkgSdkInfo.json ({})",
                    pkg_sdk_info_path.display()
                )));
            }
            // Latest SDK app_packing_tool stores pkgSdkInfo.json byte-for-byte.
            // Unlike module.json/pkgContextInfo.json, it does not run this
            // metadata through fastjson's object-key normalization.
            Some(std::fs::read(pkg_sdk_info_path)?)
        } else {
            None
        };

        if let Some(syscap_path) = &input.syscap_path {
            if !syscap_path.is_file() {
                return Err(PackError::MissingInput(format!(
                    "rpcid.sc ({})",
                    syscap_path.display()
                )));
            }
            zip_writer::add_file_stored(&mut writer, "rpcid.sc", syscap_path)?;
        }

        // Resource files — copy from src/main/resources/ into resources/ in the HAP.
        // hvigor writes them before ets/modules.abc and pack.info.
        if let Some(res_src) = &input.resources_src_dir {
            if res_src.exists() {
                for entry in walkdir::WalkDir::new(res_src)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                {
                    let rel = entry.path().strip_prefix(res_src).unwrap_or(entry.path());
                    let dest = ArchivePath::resources(rel);
                    let ext = entry
                        .path()
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if ext == "json" {
                        add_json_resource_stored(&mut writer, &dest, entry.path())?;
                    } else {
                        zip_writer::add_file_stored(&mut writer, &dest, entry.path())?;
                    }
                }
            }
        }

        // modules.abc — stored at ets/modules.abc for API >= 9, else modules.abc
        if input.abc_path.exists() {
            let abc_dest = if self.config.sdk_config.compile_api_version >= 9 {
                "ets/modules.abc"
            } else {
                "modules.abc"
            };
            zip_writer::add_file_stored(&mut writer, abc_dest, &input.abc_path)?;
        } else {
            return Err(PackError::MissingInput("modules.abc".to_string()));
        }

        if let Some(source_maps_path) = &input.source_maps_path {
            if !source_maps_path.exists() {
                return Err(PackError::MissingInput(format!(
                    "sourceMaps.map ({})",
                    source_maps_path.display()
                )));
            }
            zip_writer::add_file_stored(&mut writer, "ets/sourceMaps.map", source_maps_path)?;
        }

        // Additional ETS-side assets copied by CompileArkTS/loader, such as
        // `src/main/ets/common/**/*.json`, are packaged under `ets/`.
        if let Some(ets_dir) = input.abc_path.parent() {
            Self::add_loader_out_ets_assets(&mut writer, ets_dir, &input.output_path)?;
        }

        // Native .so libraries — placed under libs/<abi>/, with compression
        // controlled by module.json#module.compressNativeLibs.
        if input.compress_native_libs {
            let native_entries = input
                .native_libs
                .iter()
                .filter(|so_path| so_path.exists())
                .map(|so_path| (ArchivePath::native_lib(so_path), so_path.clone()))
                .collect();
            let compressed =
                zip_writer::ParallelDeflatedFiles::compress(native_entries, self.compress_level)?;
            for entry in compressed.entries() {
                writer.add_precompressed_deflated(entry)?;
            }
        } else {
            for so_path in &input.native_libs {
                if so_path.exists() {
                    let dest = ArchivePath::native_lib(so_path);
                    zip_writer::add_file_stored(&mut writer, &dest, so_path)?;
                }
            }
        }

        // Integrated (embedded) remote HSPs — stored at libs/<hsp_dir_name>/<hsp_file_name>.
        // Each HSP must have been normalized via `normalize_integrated_hsps()` before packing.
        for hsp in &input.integrated_hsps {
            if hsp.hsp_path.exists() {
                let dest = ArchivePath::integrated_hsp(&hsp.hsp_dir_name, &hsp.hsp_file_name);
                zip_writer::add_file_stored(&mut writer, &dest, &hsp.hsp_path)?;
            }
        }

        Self::add_profile_guided_ap(&mut writer, &input)?;

        // module.privacy.json — PAC/privacy profile (stored), written when
        // `ProcessModulePrivacyProfile` produced a processed privacy profile.
        // Hvigor accepts `pac.json5`; the archive entry remains JSON.
        if let Some(ref privacy_path) = input.privacy_profile_path {
            if privacy_path.exists() {
                if let Ok(content) = std::fs::read_to_string(privacy_path) {
                    if let Ok(value) = json5::from_str::<serde_json::Value>(&content) {
                        let bytes = serde_json::to_vec(&value)
                            .expect("serializing parsed privacy JSON should not fail");
                        zip_writer::add_bytes_stored(&mut writer, "module.privacy.json", &bytes)?;
                    } else {
                        tracing::warn!(
                            path = %privacy_path.display(),
                            "ProcessModulePrivacyProfile: invalid JSON5 in privacy profile; skipping"
                        );
                    }
                }
            }
        }

        // pack.info — the last regular bundle-metadata entry; the SDK appends
        // optional pkgContextInfo.json after it.
        let mut pack_info = self.generate_pack_info(&input, pack_info_module_json_value.as_ref());
        pack_info.push('\n');
        zip_writer::add_bytes_stored(&mut writer, "pack.info", pack_info.as_bytes())?;

        // The SDK package tool writes pkgContextInfo after its regular HAP
        // entries (including pack.info), even though the CLI option is supplied
        // alongside the other input paths.
        if let Some(pkg_context) = pkg_context {
            zip_writer::add_bytes_stored(&mut writer, "pkgContextInfo.json", &pkg_context)?;
        }
        if let Some(pkg_sdk) = pkg_sdk {
            zip_writer::add_bytes_stored(&mut writer, "pkgSdkInfo.json", &pkg_sdk)?;
        }

        writer.finish()?;
        Ok(input.output_path)
    }

    /// Package an HSP archive.
    ///
    /// HSP (Harmony Shared Package) has the same on-disk layout as HAP; the
    /// `module.type` field in `module.json` distinguishes them at runtime.
    pub fn pack_hsp(&self, input: PackInput) -> Result<PathBuf, PackError> {
        self.pack_hap(input)
    }

    /// Package the HSP distribution archive emitted by `PackageSharedTgz`.
    pub fn pack_hsp_tgz(&self, input: HspTgzInput) -> Result<PathBuf, PackError> {
        for source in [&input.hsp_path, &input.interface_har_path] {
            if !source.is_file() {
                return Err(PackError::MissingInput(format!(
                    "PackageSharedTgz input does not exist: {}",
                    source.display()
                )));
            }
        }
        if let Some(parent) = input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }

        let hsp_file_name = input
            .hsp_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                PackError::MissingInput(format!(
                    "PackageSharedTgz HSP file name is not valid UTF-8: {}",
                    input.hsp_path.display()
                ))
            })?;
        let hsp_archive_name = hsp_file_name
            .strip_suffix("-unsigned.hsp")
            .map(|stem| format!("{stem}.hsp"))
            .unwrap_or_else(|| hsp_file_name.to_string());
        let interface_har_name = input
            .interface_har_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                PackError::MissingInput(format!(
                    "PackageSharedTgz HAR file name is not valid UTF-8: {}",
                    input.interface_har_path.display()
                ))
            })?;

        let output = fs_err::File::create(&input.output_path)?;
        let gzip = flate2::GzBuilder::new()
            .mtime(0)
            .write(output, flate2::Compression::default());
        let mut archive = tar::Builder::new(gzip);
        for (source, archive_name) in [
            (&input.hsp_path, hsp_archive_name.as_str()),
            (&input.interface_har_path, interface_har_name),
        ] {
            let mut file = fs_err::File::open(source)?;
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(file.metadata()?.len());
            header.set_mode(0o644);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            archive.append_data(&mut header, archive_name, &mut file)?;
        }
        archive.finish()?;
        archive.into_inner()?.finish()?;
        Ok(input.output_path)
    }

    /// Package the declaration-only interface HAR emitted alongside an HSP.
    ///
    /// This follows hvigor `PrepareSharedHarRes`/`PackageSharedHar`: the
    /// processed profile is reduced to its consumer-visible fields, the
    /// package manifest is converted to `InterfaceHar`, and declarations are
    /// copied from the HSP-specific `<module>/src/main/ets` tree.
    pub fn pack_hsp_interface_har(
        &self,
        input: HspInterfaceHarInput,
    ) -> Result<PathBuf, PackError> {
        if let Some(parent) = input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }

        let manifest = HspInterfaceHarManifest::load(&input)?;
        let module_json = HspInterfaceModuleJson::load(&input.processed_module_json)?;
        let mut entries = HarArchiveEntries::new(&input.output_path);
        entries.collect_hsp_special_files(&input.module_root)?;
        entries.collect_hsp_local_dependencies(&input.module_root, &manifest.local_dependencies)?;

        let declaration_root = input.declarations_dir.join(&input.module_name);
        if !HarArchiveEntries::contains_declarations(&declaration_root) {
            return Err(PackError::MissingInput(format!(
                "HSP interface declarations do not exist below {}",
                declaration_root.display()
            )));
        }
        entries.collect_required_tree(&declaration_root, Path::new(""))?;

        if let Some(resource_table) = input.resource_table_path.as_ref() {
            entries.insert_required("ResourceTable.txt", resource_table)?;
        }
        if let Some(resource_str) = input.resource_str_dir.as_ref() {
            entries.collect_tree_if_exists(resource_str, Path::new("src/main/resource"))?;
        }
        if let Some(privacy_profile) = input.privacy_profile_path.as_ref() {
            entries.insert_if_exists("pac.json5", privacy_profile)?;
        }
        if let Some(obfuscation_rules) = input.obfuscation_rules_path.as_ref() {
            entries.insert_if_exists("obfuscation.txt", obfuscation_rules)?;
        }
        let include_native_libs =
            !input.exclude_so_from_interface_har || !input.native_header_dirs.is_empty();
        if input.exclude_so_from_interface_har && !input.native_header_dirs.is_empty() {
            tracing::warn!(
                module = %input.module_name,
                "excludeSoFromInterfaceHar is ignored when native headerPath is configured"
            );
        }
        if include_native_libs {
            for native_lib in &input.native_libs {
                entries.insert_required(ArchivePath::native_lib(native_lib), native_lib)?;
            }
        }
        for header_dir in &input.native_header_dirs {
            entries.collect_required_tree(header_dir, Path::new("include"))?;
        }

        entries.insert_bytes("oh-package.json5", manifest.serialize()?);
        entries.insert_bytes("src/main/module.json", module_json.serialize()?);
        entries.write_archive()?;
        Ok(input.output_path)
    }

    /// Produce a "cleaned" source HAR for tgz publishing.
    ///
    /// Implements hvigor's `cleanEtsForTgz` step: when a source HAR is published
    /// as a tgz package, the original `.ets` source files are replaced by the
    /// generated declaration stubs (`index.d.ts`, `index.d.ets`) so that consumers
    /// get type information without accessing the original source code.
    ///
    /// Returns `(cleaned_source_dir, declarations_dir_copy)` where
    /// `cleaned_source_dir` can be passed to `pack_har` in place of `source_dir`.
    /// The cleaned directory is written to `output_dir` on disk.
    pub fn clean_har_sources(
        source_dir: &std::path::Path,
        declarations_dir: Option<&std::path::Path>,
        output_dir: &std::path::Path,
    ) -> Result<std::path::PathBuf, PackError> {
        // Create the output directory.
        fs_err::create_dir_all(output_dir)?;

        let cleaned_ets = output_dir.join("ets");
        fs_err::create_dir_all(&cleaned_ets)?;

        // Copy declaration stubs (if present) into the cleaned ets directory.
        if let Some(decl_dir) = declarations_dir {
            for entry in walkdir::WalkDir::new(decl_dir)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                if ArchivePath::is_declaration_file(entry.path()) {
                    let relative = entry.path().strip_prefix(decl_dir).map_err(|error| {
                        PackError::MissingInput(format!(
                            "invalid HAR declaration path {}: {error}",
                            entry.path().display()
                        ))
                    })?;
                    let destination = cleaned_ets.join(relative);
                    if let Some(parent) = destination.parent() {
                        fs_err::create_dir_all(parent)?;
                    }
                    fs_err::copy(entry.path(), destination)?;
                }
            }
        }

        // Copy non-ETS source files (resources, config files) preserving structure.
        // Original .ets/.ts source files are NOT copied (cleanEtsForTgz removes them).
        if source_dir.is_dir() {
            let parent = source_dir.parent().unwrap_or(source_dir);
            for entry in walkdir::WalkDir::new(source_dir)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                // Skip original source files (they're replaced by declarations).
                if matches!(ext, "ets" | "ts") && !ArchivePath::is_declaration_file(path) {
                    continue;
                }
                // Copy everything else (JSON, resources, etc.).
                if let Ok(rel) = path.strip_prefix(parent) {
                    let dest = output_dir.join(rel);
                    if let Some(dest_parent) = dest.parent() {
                        fs_err::create_dir_all(dest_parent)?;
                    }
                    fs_err::copy(path, &dest)?;
                }
            }
        }

        Ok(cleaned_ets)
    }

    /// Package an original source HAR. Call [`Self::pack_har_variant`] when
    /// the effective target requests obfuscation or ByteCode HAR output.
    pub fn pack_har(&self, input: HarPackInput) -> Result<PathBuf, PackError> {
        let module_root = input
            .source_dir
            .ancestors()
            .nth(3)
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                PackError::MissingInput(format!(
                    "cannot derive HAR module root from {}",
                    input.source_dir.display()
                ))
            })?;
        self.pack_har_variant(
            input,
            HarVariantInput {
                artifact_type: HarArtifactType::Original,
                module_root,
                compiled_output_dir: PathBuf::new(),
                processed_module_json: None,
                resource_table_path: None,
                native_libs: Vec::new(),
                native_header_dirs: Vec::new(),
                preload_system_so_json: None,
                source_maps_path: None,
                build_profile_path: None,
                declaration_entries: Vec::new(),
                package_parameters: HarPackageParameterInput::default(),
                worker_entries: Vec::new(),
                runtime_only: None,
                resource_directories: Vec::new(),
                bundled_dependency_rewrite: None,
                dependency_pkg_version: IndexMap::new(),
                bundled_dependencies: false,
                bundled_resources: Vec::new(),
                bundled_resource_directories: Vec::new(),
                compiled_package_names: Vec::new(),
                extra_importees: Vec::new(),
                global_dependencies: Vec::new(),
                ignore_resource_patterns: Vec::new(),
                debug: true,
                target_name: "default".to_string(),
                package_cache_dir: None,
            },
        )
    }

    /// Assemble the three HAR artifact shapes owned by hvigor's
    /// `AbstractProcessHarArtifacts` and `ProcessHarArtifacts` tasks.
    pub fn pack_har_variant(
        &self,
        input: HarPackInput,
        variant: HarVariantInput,
    ) -> Result<PathBuf, PackError> {
        if let Some(parent) = input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let manifest = HarManifest::load(&input, &variant, &self.config)?;
        let mut entries = HarArchiveEntries::new(&input.output_path);
        match variant.artifact_type {
            HarArtifactType::Original => entries.collect_original_sources(
                &variant.module_root,
                &input.oh_package_json,
                manifest.main.as_deref(),
            )?,
            HarArtifactType::Obfuscation => {
                entries
                    .collect_resources(&input.resources_dir, &variant.ignore_resource_patterns)?;
                entries.collect_required_tree(&variant.compiled_output_dir, Path::new(""))?;
            }
            HarArtifactType::Bytecode => {
                entries
                    .collect_resources(&input.resources_dir, &variant.ignore_resource_patterns)?;
                entries.collect_required_tree(
                    &variant.compiled_output_dir.join("ets"),
                    Path::new("ets"),
                )?;
            }
        }
        if let Some(resource_str_dir) = input.resource_str_dir.as_ref() {
            entries.collect_tree_if_exists(resource_str_dir, Path::new("src/main/resource"))?;
        }
        if variant.bundled_dependencies {
            for resource in &variant.bundled_resources {
                entries.collect_tree_if_exists(
                    &resource.source_dir,
                    Path::new(resource.archive_directory.as_str()),
                )?;
            }
        }
        if let Some(processed_module_json) = variant.processed_module_json.as_ref() {
            entries.insert_required("src/main/module.json", processed_module_json)?;
        }
        if let Some(resource_table) = variant.resource_table_path.as_ref() {
            entries.insert_required("ResourceTable.txt", resource_table)?;
        }
        if !matches!(variant.artifact_type, HarArtifactType::Original) {
            if let Some(declarations_dir) = input.declarations_dir.as_ref() {
                entries.collect_declarations(&declarations_dir.join(&input.module_name))?;
            }
        }
        for native_lib in &variant.native_libs {
            entries.insert_required(ArchivePath::native_lib(native_lib), native_lib)?;
        }
        for header_dir in &variant.native_header_dirs {
            entries.collect_required_tree(header_dir, Path::new("include"))?;
        }
        if matches!(variant.artifact_type, HarArtifactType::Bytecode) {
            if let Some(preload) = variant.preload_system_so_json.as_ref() {
                let name = preload
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("preload.json");
                entries.insert_required(format!("META-INF/{name}"), preload)?;
            }
            if variant.debug {
                if let Some(source_maps) = variant.source_maps_path.as_ref() {
                    entries.insert_required("ets/sourceMaps.map", source_maps)?;
                }
            }
        }
        if let Some(consumer_rules) = input.consumer_rules_path.as_ref() {
            entries.insert_if_exists("obfuscation.txt", consumer_rules)?;
        }

        entries.insert_bytes("oh-package.json5", manifest.serialize()?);
        // AbstractProcessHarArtifacts copies BuildProfile.ets for source and
        // obfuscated HARs. ProcessHarArtifacts' ByteCode path stages only the
        // compiled ABC, declarations and resources.
        if !matches!(variant.artifact_type, HarArtifactType::Bytecode) {
            if let Some(build_profile_path) = variant.build_profile_path.as_ref() {
                entries.insert_required("BuildProfile.ets", build_profile_path)?;
            } else {
                entries.insert_bytes(
                    "BuildProfile.ets",
                    manifest.build_profile(&self.config, &variant),
                );
            }
        }
        // CacheNativeLibs creates this directory even when the HAR has no
        // native libraries. ohpm retains it in the package tarball.
        entries.ensure_directory("libs");
        if let Some(package_cache_dir) = variant.package_cache_dir.as_ref() {
            entries.write_directory(package_cache_dir)?;
        }
        entries.write_archive()?;

        Ok(input.output_path)
    }

    /// Materialize latest hvigor's `ProcessOhPackageJson` output under
    /// `build/<product>/generated/pm/<target>/oh-package.json5`.
    ///
    /// `ProcessHarArtifacts` consumes this file and applies its own dependency
    /// and compiler metadata later; keeping the stages separate preserves both
    /// the intermediate artifact and JavaScript object insertion order.
    pub fn process_har_package_json(
        &self,
        input: &HarPackInput,
        variant: &HarVariantInput,
        output_path: &Path,
    ) -> Result<PathBuf, PackError> {
        let manifest = ProcessedHarManifest::load(input, variant)?;
        manifest.write(output_path)?;
        Ok(output_path.to_path_buf())
    }
}

struct HspInterfaceHarManifest {
    value: serde_json::Value,
    local_dependencies: Vec<PathBuf>,
}

impl HspInterfaceHarManifest {
    fn load(input: &HspInterfaceHarInput) -> Result<Self, PackError> {
        if !input.oh_package_json.is_file() {
            return Err(PackError::MissingInput(format!(
                "HSP package manifest does not exist: {}",
                input.oh_package_json.display()
            )));
        }
        let content = fs_err::read_to_string(&input.oh_package_json)?;
        let mut value: serde_json::Value = json5::from_str(&content)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        let package = value.as_object_mut().ok_or_else(|| {
            PackError::MalformedPackInfo(format!(
                "{} must contain a JSON object",
                input.oh_package_json.display()
            ))
        })?;
        let local_dependencies = Self::local_dependencies(package);
        let main = package
            .shift_remove("main")
            .and_then(|value| value.as_str().map(str::to_string));
        package.insert(
            "types".to_string(),
            serde_json::Value::String(Self::types_from_main(main.as_deref())),
        );
        package
            .entry("packageType")
            .or_insert_with(|| serde_json::Value::String("InterfaceHar".to_string()));

        if package.contains_key("dependencies") || !input.dependencies.is_empty() {
            package.insert(
                "dependencies".to_string(),
                serde_json::to_value(&input.dependencies)
                    .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?,
            );
        }
        if package.contains_key("dynamicDependencies") || !input.dynamic_dependencies.is_empty() {
            package.insert(
                "dynamicDependencies".to_string(),
                serde_json::to_value(&input.dynamic_dependencies)
                    .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?,
            );
        }
        package.shift_remove("devDependencies");

        let metadata = package
            .entry("metadata")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
            .as_object_mut()
            .ok_or_else(|| {
                PackError::MalformedPackInfo("oh-package.json5 metadata must be an object".into())
            })?;
        metadata.insert(
            "useNormalizedOHMUrl".to_string(),
            serde_json::Value::Bool(input.use_normalized_ohm_url),
        );
        if let Some(integrated_hsp) = input.integrated_hsp {
            metadata.insert(
                "integratedHsp".to_string(),
                serde_json::Value::Bool(integrated_hsp),
            );
        }
        let mut source_roots = input.source_roots.clone();
        source_roots.push("./src/main".to_string());
        source_roots.push(format!(
            "./build/default/generated/profile/{}",
            input.target_name
        ));
        metadata.insert("sourceRoots".to_string(), serde_json::json!(source_roots));
        metadata.insert("debug".to_string(), serde_json::Value::Bool(input.debug));

        package.insert(
            "compatibleSdkVersion".to_string(),
            serde_json::Value::Number(input.compatible_sdk_version.into()),
        );
        package.insert(
            "compatibleSdkType".to_string(),
            serde_json::Value::String(input.compatible_sdk_type.clone()),
        );
        package.insert(
            "obfuscated".to_string(),
            serde_json::Value::Bool(input.obfuscated),
        );
        Ok(Self {
            value,
            local_dependencies,
        })
    }

    fn local_dependencies(package: &serde_json::Map<String, serde_json::Value>) -> Vec<PathBuf> {
        package
            .get("dependencies")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|dependencies| dependencies.values())
            .filter_map(serde_json::Value::as_str)
            .filter_map(|dependency| {
                let path = dependency.strip_prefix("file:").unwrap_or(dependency);
                let is_path = dependency.starts_with("file:")
                    || path.starts_with('.')
                    || path.starts_with('/')
                    || path.contains('/')
                    || path.contains('\\');
                is_path.then(|| PathBuf::from(path))
            })
            .collect()
    }

    fn types_from_main(main: Option<&str>) -> String {
        let Some(main) = main.filter(|main| !main.is_empty()) else {
            return "index.d.ets".to_string();
        };
        if let Some(stem) = main.strip_suffix(".ets") {
            return format!("{stem}.d.ets");
        }
        if let Some(stem) = main.strip_suffix(".ts") {
            return format!("{stem}.d.ts");
        }
        if main.ends_with(".d") {
            return main.to_string();
        }
        format!("{main}.d")
    }

    fn serialize(&self) -> Result<Vec<u8>, PackError> {
        let mut bytes = serde_json::to_vec_pretty(&self.value)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

struct HspInterfaceModuleJson {
    value: serde_json::Value,
}

impl HspInterfaceModuleJson {
    fn load(path: &Path) -> Result<Self, PackError> {
        if !path.is_file() {
            return Err(PackError::MissingInput(format!(
                "processed HSP module.json does not exist: {}",
                path.display()
            )));
        }
        let content = fs_err::read_to_string(path)?;
        let value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        let root = value.as_object().ok_or_else(|| {
            PackError::MalformedPackInfo(format!("{} must contain a JSON object", path.display()))
        })?;
        let mut output = serde_json::Map::new();
        if let Some(app) = root.get("app") {
            output.insert(
                "app".to_string(),
                Self::consumer_fields(
                    app,
                    &[
                        "bundleName",
                        "bundleType",
                        "debug",
                        "versionCode",
                        "versionName",
                        "minCompatibleVersionCode",
                        "minAPIVersion",
                        "targetAPIVersion",
                        "apiReleaseType",
                        "targetMinorAPIVersion",
                        "targetPatchAPIVersion",
                    ],
                ),
            );
        }
        if let Some(module) = root.get("module") {
            output.insert(
                "module".to_string(),
                Self::consumer_fields(
                    module,
                    &[
                        "name",
                        "packageName",
                        "type",
                        "deliveryWithInstall",
                        "deviceTypes",
                        "requestPermissions",
                        "dependencies",
                        "libIsolation",
                    ],
                ),
            );
        }
        Ok(Self {
            value: serde_json::Value::Object(output),
        })
    }

    fn consumer_fields(value: &serde_json::Value, fields: &[&str]) -> serde_json::Value {
        let Some(object) = value.as_object() else {
            return value.clone();
        };
        let mut output = serde_json::Map::new();
        for field in fields {
            if let Some(value) = object.get(*field).filter(|value| !value.is_null()) {
                output.insert((*field).to_string(), value.clone());
            }
        }
        serde_json::Value::Object(output)
    }

    fn serialize(&self) -> Result<Vec<u8>, PackError> {
        let mut bytes = serde_json::to_vec(&self.value)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

struct HarManifest {
    value: serde_json::Value,
    main: Option<String>,
}

impl HarManifest {
    fn load(
        input: &HarPackInput,
        variant: &HarVariantInput,
        config: &ArkdownConfig,
    ) -> Result<Self, PackError> {
        let processed = ProcessedHarManifest::load(input, variant)?;
        let main = processed.main;
        let mut value = processed.value;
        let package = value.as_object_mut().ok_or_else(|| {
            PackError::MalformedPackInfo(format!(
                "{} must contain a JSON object",
                input.oh_package_json.display()
            ))
        })?;
        Self::apply_bundled_dependencies(package, variant);
        let metadata = package
            .entry("metadata")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
            .as_object_mut()
            .ok_or_else(|| {
                PackError::MalformedPackInfo("oh-package.json5 metadata must be an object".into())
            })?;
        metadata.insert("sourceRoots".to_string(), serde_json::json!(["./src/main"]));
        metadata.insert("debug".to_string(), serde_json::Value::Bool(variant.debug));
        if matches!(variant.artifact_type, HarArtifactType::Bytecode) {
            metadata.insert(
                "dependencyPkgVersion".to_string(),
                serde_json::to_value(&variant.dependency_pkg_version).map_err(|error| {
                    PackError::MalformedPackInfo(format!(
                        "serialize ByteCode HAR dependencyPkgVersion failed: {error}"
                    ))
                })?,
            );
            if variant.bundled_dependencies {
                metadata.insert(
                    "bundleDependencies".to_string(),
                    serde_json::Value::Bool(true),
                );
                if !variant.bundled_resource_directories.is_empty() {
                    metadata.insert(
                        "resource".to_string(),
                        serde_json::json!({
                            "directories": variant
                                .bundled_resource_directories
                                .iter()
                                .map(ArtifactPath::as_str)
                                .collect::<Vec<_>>()
                        }),
                    );
                }
            }
            metadata.shift_remove("workers");
            if let Some(runtime_only) = metadata
                .get_mut("runtimeOnly")
                .and_then(serde_json::Value::as_object_mut)
            {
                runtime_only.shift_remove("sources");
            }
            let declarations = variant
                .declaration_entries
                .iter()
                .filter(|entry| !entry.is_empty())
                .cloned()
                .collect::<BTreeSet<_>>();
            metadata.insert(
                "declarationEntry".to_string(),
                serde_json::Value::Array(
                    declarations
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
            if !variant.global_dependencies.is_empty() {
                metadata.insert(
                    "globalDependencies".to_string(),
                    serde_json::json!(&variant.global_dependencies),
                );
            }
            if !variant.extra_importees.is_empty() {
                metadata.insert(
                    "extraImportees".to_string(),
                    serde_json::json!(&variant.extra_importees),
                );
            }
            if variant.bundled_dependencies {
                metadata.insert(
                    "compiledPackageNames".to_string(),
                    serde_json::json!(&variant.compiled_package_names),
                );
                // Bundled HARs always serialize this field, including `[]`.
                metadata.insert(
                    "extraImportees".to_string(),
                    serde_json::json!(&variant.extra_importees),
                );
            }
        }
        // AbstractProcessHarArtifacts invokes FileUtil.addParamToOhPack only
        // after ProcessHarArtifacts has written its dependency/declaration
        // metadata. Keeping this last reproduces JavaScript insertion order.
        metadata.insert(
            "useNormalizedOHMUrl".to_string(),
            serde_json::Value::Bool(
                config
                    .product
                    .build_option
                    .strict_mode
                    .use_normalized_ohm_url,
            ),
        );
        package.insert(
            "compatibleSdkVersion".to_string(),
            serde_json::Value::from(Self::compatible_api_version(config)),
        );
        package.insert(
            "compatibleSdkType".to_string(),
            serde_json::Value::String(config.product.runtime_os.clone()),
        );
        package.insert(
            "obfuscated".to_string(),
            serde_json::Value::Bool(matches!(
                variant.artifact_type,
                HarArtifactType::Obfuscation
            )),
        );
        Ok(Self { value, main })
    }

    fn apply_bundled_dependencies(
        package: &mut serde_json::Map<String, serde_json::Value>,
        variant: &HarVariantInput,
    ) {
        if !variant.bundled_dependencies {
            return;
        }
        package.shift_remove("devDependencies");
        let Some(rewrite) = variant.bundled_dependency_rewrite.as_ref() else {
            return;
        };
        if package.contains_key("dependencies") {
            package.insert(
                "dependencies".to_string(),
                serde_json::Value::Object(
                    rewrite
                        .dependencies
                        .iter()
                        .map(|(name, version)| {
                            (name.clone(), serde_json::Value::String(version.clone()))
                        })
                        .collect(),
                ),
            );
        }
        if package.contains_key("dynamicDependencies") {
            package.insert(
                "dynamicDependencies".to_string(),
                serde_json::Value::Object(
                    rewrite
                        .dynamic_dependencies
                        .iter()
                        .map(|(name, version)| {
                            (name.clone(), serde_json::Value::String(version.clone()))
                        })
                        .collect(),
                ),
            );
        }
    }

    fn declaration_path_for_main(main: &str) -> String {
        for extension in [".d.ets", ".d.ts", ".ets", ".ts", ".js"] {
            if let Some(stem) = main.strip_suffix(extension) {
                return format!(
                    "{stem}.d.{}",
                    if extension.ends_with("ets") {
                        "ets"
                    } else {
                        "ts"
                    }
                );
            }
        }
        format!("{main}.d.ets")
    }

    fn compatible_api_version(config: &ArkdownConfig) -> u32 {
        config
            .product
            .compatible_sdk_version
            .rsplit_once('(')
            .and_then(|(_, version)| version.strip_suffix(')'))
            .and_then(|version| version.parse().ok())
            .unwrap_or(config.sdk_config.compile_api_version)
    }

    /// Mirrors hvigor `HarBuildProfileTask` and
    /// `build-profile-utils.normalizedFileData` for the built-in fields.
    fn build_profile(&self, config: &ArkdownConfig, variant: &HarVariantInput) -> Vec<u8> {
        let version = self
            .value
            .get("version")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let mode = match config.build_mode {
            arkdown_core::BuildMode::Debug => "debug",
            arkdown_core::BuildMode::Release => "release",
        };
        format!(
            "/**\n * Use these variables when you tailor your ArkTS code. They must be of the const type.\n */\n\
export const HAR_VERSION = '{version}';\n\
export const BUILD_MODE_NAME = '{mode}';\n\
export const DEBUG = {};\n\
export const TARGET_NAME = '{}';\n\n\
/**\n * BuildProfile Class is used only for compatibility purposes.\n */\n\
export default class BuildProfile {{ \n\
\tstatic readonly HAR_VERSION = HAR_VERSION;\n\
\tstatic readonly BUILD_MODE_NAME = BUILD_MODE_NAME;\n\
\tstatic readonly DEBUG = DEBUG;\n\
\tstatic readonly TARGET_NAME = TARGET_NAME;\n\
}}",
            variant.debug, variant.target_name
        )
        .into_bytes()
    }

    fn serialize(&self) -> Result<Vec<u8>, PackError> {
        let mut bytes = serde_json::to_vec(&self.value)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        // fs-extra.outputJSONSync, used by hvigor before `ohpm pack`, writes a
        // trailing newline even when the package object is otherwise minified.
        bytes.push(b'\n');
        Ok(bytes)
    }
}

struct ProcessedHarManifest {
    value: serde_json::Value,
    main: Option<String>,
}

impl ProcessedHarManifest {
    fn load(input: &HarPackInput, variant: &HarVariantInput) -> Result<Self, PackError> {
        if !input.oh_package_json.is_file() {
            return Err(PackError::MissingInput(format!(
                "HAR manifest does not exist: {}",
                input.oh_package_json.display()
            )));
        }
        let content = fs_err::read_to_string(&input.oh_package_json)?;
        let mut value: serde_json::Value = json5::from_str(&content)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        let package = value.as_object_mut().ok_or_else(|| {
            PackError::MalformedPackInfo(format!(
                "{} must contain a JSON object",
                input.oh_package_json.display()
            ))
        })?;
        Self::apply_package_parameters(package, &variant.package_parameters);
        let main = package
            .get("main")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        if !matches!(variant.artifact_type, HarArtifactType::Original) {
            let valid_main = Self::valid_main(&variant.module_root, main.as_deref())?;
            if !valid_main.ends_with(".js") {
                package.entry("types").or_insert_with(|| {
                    serde_json::Value::String(HarManifest::declaration_path_for_main(&valid_main))
                });
            }
            package.insert(
                "main".to_string(),
                serde_json::Value::String(Self::compiled_main(&valid_main)),
            );
            // `CoreProcessPackageJson.addArtifactType` uses `obfuscation` for
            // both obfuscated and ByteCode HARs in hvigor 6.24.3.
            package.insert(
                "artifactType".to_string(),
                serde_json::Value::String("obfuscation".to_string()),
            );
        }

        Self::apply_workers(package, variant)?;
        Self::apply_runtime_only(package, variant)?;
        if matches!(variant.artifact_type, HarArtifactType::Bytecode) {
            Self::metadata(package)?
                .insert("byteCodeHar".to_string(), serde_json::Value::Bool(true));
            // JavaScript `delete package.main` preserves all remaining key
            // positions. `shift_remove` provides the same IndexMap behavior.
            package.shift_remove("main");
        }
        Self::apply_resource_directories(package, variant)?;
        Ok(Self { value, main })
    }

    fn write(&self, output_path: &Path) -> Result<(), PackError> {
        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut bytes = serde_json::to_vec(&self.value)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        bytes.push(b'\n');
        fs_err::write(output_path, bytes)?;
        Ok(())
    }

    fn valid_main(module_root: &Path, configured: Option<&str>) -> Result<String, PackError> {
        if let Some(main) = configured.filter(|main| !main.is_empty()) {
            let path = module_root.join(main);
            if !path.is_file() {
                return Err(PackError::MissingInput(format!(
                    "HAR main field '{}' does not exist: {}",
                    main,
                    path.display()
                )));
            }
            return Ok(main.to_string());
        }
        let mut found = None;
        for candidate in ["index.ets", "index.ts", "index.js"] {
            if module_root.join(candidate).is_file() {
                found = Some(candidate.to_string());
            }
        }
        Ok(found.unwrap_or_else(|| "index.ets".to_string()))
    }

    fn compiled_main(main: &str) -> String {
        let path = Path::new(main);
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("ets" | "ts" | "js") => {
                ArtifactPathSerializer::rel_to_slash(&path.with_extension("js"))
            }
            _ => format!("{main}.js"),
        }
    }

    fn apply_workers(
        package: &mut serde_json::Map<String, serde_json::Value>,
        variant: &HarVariantInput,
    ) -> Result<(), PackError> {
        if variant.worker_entries.is_empty() {
            return Ok(());
        }
        let compiled = !matches!(variant.artifact_type, HarArtifactType::Original);
        let mut workers = Vec::with_capacity(variant.worker_entries.len());
        for worker in &variant.worker_entries {
            let relative = worker.strip_prefix(&variant.module_root).map_err(|_| {
                PackError::MalformedPackInfo(format!(
                    "HAR worker entry is outside module root: {} not under {}",
                    worker.display(),
                    variant.module_root.display()
                ))
            })?;
            let mut relative = relative.to_path_buf();
            if compiled {
                relative.set_extension("js");
            }
            workers.push(serde_json::Value::String(
                ArtifactPathSerializer::rel_to_slash(&relative),
            ));
        }
        Self::metadata(package)?.insert("workers".to_string(), serde_json::Value::Array(workers));
        Ok(())
    }

    fn apply_runtime_only(
        package: &mut serde_json::Map<String, serde_json::Value>,
        variant: &HarVariantInput,
    ) -> Result<(), PackError> {
        let Some(runtime_only) = variant.runtime_only.as_ref() else {
            return Ok(());
        };
        let open_source = matches!(variant.artifact_type, HarArtifactType::Original);
        let mut sources = Vec::new();
        for source in &runtime_only.sources {
            if !(source.starts_with("./") || source.starts_with("../")) {
                continue;
            }
            let source_path = variant.module_root.join(source);
            let metadata = fs_err::metadata(&source_path).map_err(|error| {
                PackError::MissingInput(format!(
                    "HAR runtimeOnly source does not exist: {}: {}",
                    source_path.display(),
                    error
                ))
            })?;
            let output = if metadata.is_dir()
                || open_source
                || Path::new(source)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    == Some("js")
            {
                source.clone()
            } else {
                ArtifactPathSerializer::rel_to_slash(&Path::new(source).with_extension("js"))
            };
            sources.push(serde_json::Value::String(output));
        }
        Self::metadata(package)?.insert(
            "runtimeOnly".to_string(),
            serde_json::json!({
                "sources": sources,
                "packages": runtime_only.packages,
            }),
        );
        Ok(())
    }

    fn apply_resource_directories(
        package: &mut serde_json::Map<String, serde_json::Value>,
        variant: &HarVariantInput,
    ) -> Result<(), PackError> {
        let directories = variant
            .resource_directories
            .iter()
            .filter_map(|directory| directory.strip_prefix(&variant.module_root).ok())
            .map(ArtifactPathSerializer::path_to_slash_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        if !directories.is_empty() {
            Self::metadata(package)?.insert(
                "resource".to_string(),
                serde_json::json!({ "directories": directories }),
            );
        }
        Ok(())
    }

    fn metadata(
        package: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<&mut serde_json::Map<String, serde_json::Value>, PackError> {
        package
            .entry("metadata")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
            .as_object_mut()
            .ok_or_else(|| {
                PackError::MalformedPackInfo("oh-package.json5 metadata must be an object".into())
            })
    }

    fn apply_package_parameters(
        package: &mut serde_json::Map<String, serde_json::Value>,
        parameters: &HarPackageParameterInput,
    ) {
        if let (Some(slot), Some(version)) = (package.get_mut("version"), &parameters.version) {
            *slot = serde_json::Value::String(version.clone());
        }
        Self::apply_dependency_parameters(package, "dependencies", &parameters.dependencies);
        Self::apply_dependency_parameters(package, "devDependencies", &parameters.dev_dependencies);
        Self::apply_dependency_parameters(
            package,
            "dynamicDependencies",
            &parameters.dynamic_dependencies,
        );
    }

    fn apply_dependency_parameters(
        package: &mut serde_json::Map<String, serde_json::Value>,
        field: &str,
        parameters: &IndexMap<String, String>,
    ) {
        let Some(dependencies) = package
            .get_mut(field)
            .and_then(serde_json::Value::as_object_mut)
        else {
            return;
        };
        for (name, version) in parameters {
            if let Some(slot) = dependencies.get_mut(name) {
                *slot = serde_json::Value::String(version.clone());
            }
        }
    }
}

struct HarArchiveEntries<'a> {
    output_path: &'a Path,
    files: BTreeMap<String, PathBuf>,
    inline_files: BTreeMap<String, Vec<u8>>,
    directories: BTreeSet<String>,
}

impl<'a> HarArchiveEntries<'a> {
    const INVALID_TOP_LEVEL: [&'static str; 9] = [
        "libs",
        "build",
        "node_modules",
        "oh_modules",
        ".cxx",
        ".previewer",
        ".hvigor",
        ".gitignore",
        ".ohpmignore",
    ];

    fn new(output_path: &'a Path) -> Self {
        Self {
            output_path,
            files: BTreeMap::new(),
            inline_files: BTreeMap::new(),
            directories: BTreeSet::new(),
        }
    }

    fn collect_original_sources(
        &mut self,
        module_root: &Path,
        package_manifest: &Path,
        main: Option<&str>,
    ) -> Result<(), PackError> {
        if !module_root.is_dir() {
            return Err(PackError::MissingInput(format!(
                "HAR module root does not exist: {}",
                module_root.display()
            )));
        }
        let ignore_rules = HarIgnoreRules::load(module_root)?;
        let main = main.map(|path| {
            ArtifactPathSerializer::rel_to_slash(Path::new(path.trim_start_matches("./")))
        });
        let mut files = Self::files_under(module_root)?;
        files.sort();
        for path in files {
            let rel = path.strip_prefix(module_root).map_err(|error| {
                PackError::MissingInput(format!(
                    "failed to relativize {} to {}: {error}",
                    path.display(),
                    module_root.display()
                ))
            })?;
            let rel_text = ArtifactPathSerializer::rel_to_slash(rel);
            let top = rel.components().next().and_then(|part| match part {
                std::path::Component::Normal(name) => name.to_str(),
                _ => None,
            });
            if top.is_some_and(|name| Self::INVALID_TOP_LEVEL.contains(&name))
                || path == package_manifest
                || rel == Path::new("src/main/module.json5")
                || rel_text.starts_with("src/test/")
                || rel_text.starts_with("src/ohosTest/")
            {
                continue;
            }
            if main.as_deref() != Some(rel_text.as_str()) && ignore_rules.is_ignored(&rel_text) {
                continue;
            }
            self.files.insert(rel_text, path);
        }
        Ok(())
    }

    fn collect_hsp_special_files(&mut self, module_root: &Path) -> Result<(), PackError> {
        if !module_root.is_dir() {
            return Err(PackError::MissingInput(format!(
                "HSP module root does not exist: {}",
                module_root.display()
            )));
        }
        for entry in fs_err::read_dir(module_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            let stem = lower.split('.').next().unwrap_or_default();
            let is_readme = lower == "readme.opensource"
                || matches!(
                    lower.as_str(),
                    "readme.md" | "readme-cn.md" | "readme_cn.md" | "readme-en.md" | "readme_en.md"
                );
            let is_changelog = matches!(
                lower.as_str(),
                "changelog.md"
                    | "changelog-cn.md"
                    | "changelog_cn.md"
                    | "changelog-en.md"
                    | "changelog_en.md"
            );
            if stem == "license" || is_readme || is_changelog {
                self.files.insert(name, entry.path());
            }
        }
        Ok(())
    }

    fn collect_hsp_local_dependencies(
        &mut self,
        module_root: &Path,
        dependencies: &[PathBuf],
    ) -> Result<(), PackError> {
        let canonical_root = AbsPath::canonicalize(module_root)?.into_path_buf();
        for dependency in dependencies {
            let source = if dependency.has_root() {
                dependency.clone()
            } else {
                module_root.join(dependency)
            };
            if !source.exists() {
                continue;
            }
            let canonical_source = AbsPath::canonicalize(&source)?.into_path_buf();
            if !canonical_source.starts_with(&canonical_root) || canonical_source == canonical_root
            {
                continue;
            }
            let relative = canonical_source
                .strip_prefix(&canonical_root)
                .map_err(|error| {
                    PackError::MissingInput(format!(
                        "invalid local HSP dependency {}: {error}",
                        source.display()
                    ))
                })?;
            if canonical_source.is_file() {
                self.files.insert(
                    ArtifactPathSerializer::rel_to_slash(relative),
                    canonical_source,
                );
            } else if canonical_source.is_dir() {
                self.collect_tree(&canonical_source, relative)?;
            }
        }
        Ok(())
    }

    fn collect_resources(&mut self, root: &Path, patterns: &[String]) -> Result<(), PackError> {
        let filter = HarResourceFilter::new(patterns)?;
        for path in Self::files_under_if_exists(root)? {
            if filter.is_ignored(&path) {
                continue;
            }
            let rel = path.strip_prefix(root).map_err(|error| {
                PackError::MissingInput(format!("invalid HAR resource path: {error}"))
            })?;
            self.files.insert(ArchivePath::har_resources(rel), path);
        }
        Ok(())
    }

    fn collect_required_tree(&mut self, root: &Path, destination: &Path) -> Result<(), PackError> {
        if !root.is_dir() {
            return Err(PackError::MissingInput(format!(
                "required HAR intermediate directory does not exist: {}",
                root.display()
            )));
        }
        self.collect_tree(root, destination)
    }

    fn collect_tree_if_exists(&mut self, root: &Path, destination: &Path) -> Result<(), PackError> {
        if root.is_dir() {
            self.collect_tree(root, destination)?;
        }
        Ok(())
    }

    fn collect_tree(&mut self, root: &Path, destination: &Path) -> Result<(), PackError> {
        for path in Self::files_under(root)? {
            let rel = path.strip_prefix(root).map_err(|error| {
                PackError::MissingInput(format!("invalid HAR intermediate path: {error}"))
            })?;
            let archive = destination.join(rel);
            self.files
                .insert(ArtifactPathSerializer::rel_to_slash(&archive), path);
        }
        Ok(())
    }

    fn collect_declarations(&mut self, root: &Path) -> Result<(), PackError> {
        for path in Self::files_under_if_exists(root)? {
            if !ArchivePath::is_declaration_file(&path) {
                continue;
            }
            let rel = path.strip_prefix(root).map_err(|error| {
                PackError::MissingInput(format!("invalid HAR declaration path: {error}"))
            })?;
            // Latest `ProcessHarArtifacts.copyDeclareFileToTempDir` keeps
            // consumer source declarations but does not package declarations
            // emitted for generated build helpers such as ByteCode HAR's
            // `build/.../ModuleInfo.ts`. Those helpers still remain under
            // `etsFortgz` for compiler bookkeeping.
            if rel.starts_with("build") {
                continue;
            }
            self.files
                .insert(ArtifactPathSerializer::rel_to_slash(rel), path);
        }
        Ok(())
    }

    fn contains_declarations(root: &Path) -> bool {
        walkdir::WalkDir::new(root)
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .any(|entry| {
                entry.file_type().is_file() && ArchivePath::is_declaration_file(entry.path())
            })
    }

    fn insert_required(
        &mut self,
        archive_path: impl Into<String>,
        source: &Path,
    ) -> Result<(), PackError> {
        if !source.is_file() {
            return Err(PackError::MissingInput(format!(
                "required HAR input does not exist: {}",
                source.display()
            )));
        }
        self.files.insert(archive_path.into(), source.to_path_buf());
        Ok(())
    }

    fn insert_if_exists(
        &mut self,
        archive_path: impl Into<String>,
        source: &Path,
    ) -> Result<(), PackError> {
        if source.exists() && !source.is_file() {
            return Err(PackError::MissingInput(format!(
                "HAR input is not a file: {}",
                source.display()
            )));
        }
        if source.is_file() {
            self.files.insert(archive_path.into(), source.to_path_buf());
        }
        Ok(())
    }

    fn insert_bytes(&mut self, archive_path: impl Into<String>, bytes: Vec<u8>) {
        self.inline_files.insert(archive_path.into(), bytes);
    }

    fn ensure_directory(&mut self, archive_path: impl Into<String>) {
        self.directories.insert(archive_path.into());
    }

    fn files_under(root: &Path) -> Result<Vec<PathBuf>, PackError> {
        let mut files = Vec::new();
        for entry in walkdir::WalkDir::new(root).min_depth(1) {
            let entry = entry?;
            if entry.file_type().is_file() {
                files.push(entry.into_path());
            }
        }
        files.sort();
        Ok(files)
    }

    fn files_under_if_exists(root: &Path) -> Result<Vec<PathBuf>, PackError> {
        if root.is_dir() {
            Self::files_under(root)
        } else {
            Ok(Vec::new())
        }
    }

    fn write_directory(&mut self, output_dir: &Path) -> Result<(), PackError> {
        if output_dir.exists() {
            if !output_dir.is_dir() {
                return Err(PackError::MissingInput(format!(
                    "HAR package cache path is not a directory: {}",
                    output_dir.display()
                )));
            }
            fs_err::remove_dir_all(output_dir)?;
        }
        fs_err::create_dir_all(output_dir)?;
        self.collect_parent_directories();

        for directory in &self.directories {
            fs_err::create_dir_all(output_dir.join(directory))?;
        }
        for (archive_path, bytes) in &self.inline_files {
            let destination = output_dir.join(archive_path);
            if let Some(parent) = destination.parent() {
                fs_err::create_dir_all(parent)?;
            }
            fs_err::write(destination, bytes)?;
        }
        for (archive_path, source) in &self.files {
            let destination = output_dir.join(archive_path);
            if let Some(parent) = destination.parent() {
                fs_err::create_dir_all(parent)?;
            }
            fs_err::copy(source, destination)?;
        }
        Ok(())
    }

    /// Write the OHPM package shape used by hvigor `execOhpmPack`: a gzip
    /// compressed tar archive whose members live below `package/`.
    fn write_archive(mut self) -> Result<(), PackError> {
        use std::io::Cursor;

        let output_canonical = AbsPath::canonicalize(self.output_path)
            .ok()
            .map(AbsPath::into_path_buf);
        self.collect_parent_directories();

        let output = fs_err::File::create(self.output_path)?;
        let gzip = flate2::GzBuilder::new()
            .mtime(0)
            .write(output, flate2::Compression::default());
        let mut archive = tar::Builder::new(gzip);

        // `ohpm pack`, called by hvigor's execOhpmPack, emits a breadth-first
        // directory traversal. Siblings are bytewise lexical, but children of
        // an earlier sibling stay ahead of children of a later sibling; this
        // differs from globally sorting every path at the same depth.
        for archive_path in self.ordered_member_paths() {
            if self.directories.contains(archive_path) {
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Directory);
                header.set_size(0);
                header.set_mode(0o755);
                header.set_uid(0);
                header.set_gid(0);
                header.set_mtime(0);
                header.set_cksum();
                archive.append_data(
                    &mut header,
                    format!("package/{archive_path}/"),
                    std::io::empty(),
                )?;
                continue;
            }
            if let Some(bytes) = self.inline_files.get(archive_path) {
                let mut header = Self::file_header(bytes.len() as u64);
                archive.append_data(
                    &mut header,
                    format!("package/{archive_path}"),
                    Cursor::new(bytes),
                )?;
                continue;
            }
            let Some(source) = self.files.get(archive_path) else {
                continue;
            };
            if source == self.output_path
                || output_canonical.as_ref().is_some_and(|output| {
                    AbsPath::canonicalize(source)
                        .ok()
                        .is_some_and(|path| path.as_path() == output)
                })
            {
                continue;
            }
            let mut input = fs_err::File::open(source)?;
            let mut header = Self::file_header(input.metadata()?.len());
            archive.append_data(&mut header, format!("package/{archive_path}"), &mut input)?;
        }
        archive.finish()?;
        archive.into_inner()?.finish()?;
        Ok(())
    }

    fn ordered_member_paths(&self) -> Vec<&str> {
        let paths = self
            .directories
            .iter()
            .map(String::as_str)
            .chain(self.inline_files.keys().map(String::as_str))
            .chain(self.files.keys().map(String::as_str))
            .collect::<BTreeSet<_>>();
        let mut children = BTreeMap::<&str, Vec<&str>>::new();
        for path in paths {
            let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
            children.entry(parent).or_default().push(path);
        }
        for siblings in children.values_mut() {
            siblings.sort_unstable_by_key(|path| path.rsplit('/').next().unwrap_or(path));
        }

        let mut ordered = Vec::new();
        let mut pending = std::collections::VecDeque::from([""]);
        while let Some(parent) = pending.pop_front() {
            let Some(siblings) = children.get(parent) else {
                continue;
            };
            for path in siblings {
                ordered.push(*path);
                if self.directories.contains(*path) {
                    pending.push_back(*path);
                }
            }
        }
        ordered
    }

    fn collect_parent_directories(&mut self) {
        let paths = self
            .files
            .keys()
            .chain(self.inline_files.keys())
            .cloned()
            .collect::<Vec<_>>();
        for path in paths {
            let mut parent = Path::new(&path).parent();
            while let Some(directory) = parent.filter(|directory| !directory.as_os_str().is_empty())
            {
                self.directories
                    .insert(ArtifactPathSerializer::rel_to_slash(directory));
                parent = directory.parent();
            }
        }
    }

    fn file_header(size: u64) -> tar::Header {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(size);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        header
    }
}

struct HarIgnoreRules {
    rules: Vec<(bool, globset::GlobMatcher)>,
}

impl HarIgnoreRules {
    fn load(module_root: &Path) -> Result<Self, PackError> {
        let mut rules = Vec::new();
        for file_name in [".gitignore", ".ohpmignore"] {
            let path = module_root.join(file_name);
            if !path.is_file() {
                continue;
            }
            for line in fs_err::read_to_string(path)?.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let (negated, pattern) = line
                    .strip_prefix('!')
                    .map_or((false, line), |pattern| (true, pattern));
                for matcher in Self::matchers(pattern)? {
                    rules.push((negated, matcher));
                }
            }
        }
        Ok(Self { rules })
    }

    fn matchers(pattern: &str) -> Result<Vec<globset::GlobMatcher>, PackError> {
        let pattern =
            ArtifactPathSerializer::rel_to_slash(Path::new(pattern.trim_start_matches('/')));
        let mut patterns = vec![pattern.clone()];
        if !pattern.contains('/') {
            patterns.push(format!("**/{pattern}"));
        }
        if !pattern.ends_with("/**") {
            patterns.push(format!("{pattern}/**"));
        }
        patterns
            .into_iter()
            .map(|pattern| {
                globset::Glob::new(&pattern)
                    .map(|glob| glob.compile_matcher())
                    .map_err(|error| PackError::InvalidPattern {
                        pattern,
                        message: error.to_string(),
                    })
            })
            .collect()
    }

    fn is_ignored(&self, relative_path: &str) -> bool {
        let mut ignored = false;
        for (negated, matcher) in &self.rules {
            if matcher.is_match(relative_path) {
                ignored = !negated;
            }
        }
        ignored
    }
}

struct HarResourceFilter {
    patterns: Vec<globset::GlobMatcher>,
}

impl HarResourceFilter {
    fn new(patterns: &[String]) -> Result<Self, PackError> {
        let patterns = patterns
            .iter()
            .map(|pattern| {
                globset::Glob::new(pattern)
                    .map(|glob| glob.compile_matcher())
                    .map_err(|error| PackError::InvalidPattern {
                        pattern: pattern.clone(),
                        message: error.to_string(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { patterns })
    }

    fn is_ignored(&self, path: &Path) -> bool {
        path.file_name()
            .is_some_and(|name| self.patterns.iter().any(|pattern| pattern.is_match(name)))
    }
}

// Private helpers for HAP/HSP assembly
impl Packager {
    fn add_loader_out_ets_assets<W: std::io::Write + std::io::Seek>(
        writer: &mut zip_writer::RawZipWriter<W>,
        ets_dir: &std::path::Path,
        output_path: &std::path::Path,
    ) -> Result<(), PackError> {
        let output_canonical = AbsPath::canonicalize(output_path)
            .ok()
            .map(AbsPath::into_path_buf);
        for entry in walkdir::WalkDir::new(ets_dir)
            .min_depth(1)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            let rel = path.strip_prefix(ets_dir).unwrap_or(path);
            let rel_text = ArtifactPathSerializer::rel_to_slash(rel);
            if LoaderOutEtsAsset::new(path, rel, output_path, output_canonical.as_deref())
                .is_generated_artifact()
            {
                continue;
            }
            let dest = ArchivePath::ets(&rel_text);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "json" {
                add_json_resource_stored(writer, &dest, path)?;
            } else {
                zip_writer::add_file_stored(writer, &dest, path)?;
            }
        }
        Ok(())
    }

    fn add_profile_guided_ap<W: std::io::Write + std::io::Seek>(
        writer: &mut zip_writer::RawZipWriter<W>,
        input: &PackInput,
    ) -> Result<(), PackError> {
        let Some(output_parent) = input.output_path.parent() else {
            return Ok(());
        };
        let Some(module_root) = output_parent.ancestors().nth(4) else {
            return Ok(());
        };
        let ap_file = ArchivePath::profile_guided_ap_file(&input.module_name);
        let hvigor_ap_path = module_root
            .join("build/default/intermediates/ap")
            .join(&ap_file);
        let legacy_ap_path = module_root.join(ap_file);
        let ap_path = if hvigor_ap_path.exists() {
            hvigor_ap_path
        } else {
            legacy_ap_path
        };
        if ap_path.exists() {
            zip_writer::add_file_stored(
                writer,
                &ArchivePath::profile_guided_ap(&input.module_name),
                &ap_path,
            )?;
        }
        Ok(())
    }
}

struct LoaderOutEtsAsset<'a> {
    path: &'a Path,
    rel: &'a Path,
    output_path: &'a Path,
    output_canonical: Option<&'a Path>,
}

impl<'a> LoaderOutEtsAsset<'a> {
    fn new(
        path: &'a Path,
        rel: &'a Path,
        output_path: &'a Path,
        output_canonical: Option<&'a Path>,
    ) -> Self {
        Self {
            path,
            rel,
            output_path,
            output_canonical,
        }
    }

    fn is_generated_artifact(&self) -> bool {
        self.rel == Path::new("modules.abc")
            || self.rel == Path::new("sourceMaps.map")
            || self.rel == Path::new("filesInfo.txt")
            || self.rel == Path::new("npmEntries.txt")
            || self.rel == Path::new("npmEntries.protoBin")
            || self.is_cache_artifact()
            || self.is_current_output()
            || arkdown_path::SourceFileName::new(self.path).has_extension("js")
    }

    fn is_cache_artifact(&self) -> bool {
        matches!(
            self.rel.components().next(),
            Some(std::path::Component::Normal(name)) if name == "cache"
        )
    }

    fn is_current_output(&self) -> bool {
        if self.path == self.output_path {
            return true;
        }
        self.output_canonical
            .and_then(|output| {
                AbsPath::canonicalize(self.path)
                    .ok()
                    .map(|path| path.as_path() == output)
            })
            .unwrap_or(false)
    }
}

struct ArchivePath;

impl ArchivePath {
    fn resources(rel: &Path) -> String {
        Self::prefixed_path("resources", rel)
    }

    fn native_lib(path: &Path) -> String {
        const NATIVE_ABI_DIRS: &[&str] = &["arm64-v8a", "armeabi-v7a", "x86_64", "x86", "armeabi"];

        let components = path
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(name) => name.to_str(),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let Some(index) = components
            .iter()
            .position(|component| NATIVE_ABI_DIRS.contains(component))
        {
            let mut rel = PathBuf::from("libs");
            for component in &components[index..] {
                rel.push(component);
            }
            return ArtifactPathSerializer::rel_to_slash(&rel);
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.so");
        ArtifactPathSerializer::rel_to_slash(
            &PathBuf::from("libs").join("arm64-v8a").join(filename),
        )
    }

    fn integrated_hsp(dir_name: &str, file_name: &str) -> String {
        ArtifactPathSerializer::rel_to_slash(&PathBuf::from("libs").join(dir_name).join(file_name))
    }

    fn har_resources(rel: &Path) -> String {
        Self::prefixed_path("src/main/resources", rel)
    }

    fn ets(rel_text: &str) -> String {
        ArtifactPathSerializer::rel_to_slash(&PathBuf::from("ets").join(rel_text))
    }

    fn profile_guided_ap(module_name: &str) -> String {
        ArtifactPathSerializer::rel_to_slash(
            &PathBuf::from("ap").join(Self::profile_guided_ap_file(module_name)),
        )
    }

    fn profile_guided_ap_file(module_name: &str) -> String {
        ProfileGuidedApFileName::new(module_name).file_name()
    }

    fn prefixed_path(prefix: &str, rel: &Path) -> String {
        ArtifactPathSerializer::rel_to_slash(&PathBuf::from(prefix).join(rel))
    }

    fn is_declaration_file(path: &Path) -> bool {
        arkdown_path::SourceFileName::new(path).is_declaration()
    }
}

struct ProfileResourceFileName<'a> {
    profile_name: &'a str,
}

impl<'a> ProfileResourceFileName<'a> {
    fn new(profile_name: &'a str) -> Self {
        Self { profile_name }
    }

    fn file_name(&self) -> PathBuf {
        PathBuf::from(self.profile_name).with_extension("json")
    }
}

struct ProfileGuidedApFileName<'a> {
    module_name: &'a str,
}

impl<'a> ProfileGuidedApFileName<'a> {
    fn new(module_name: &'a str) -> Self {
        Self { module_name }
    }

    fn file_name(&self) -> String {
        ArtifactPath::from_slash(self.module_name)
            .with_extension("ap")
            .into_string()
    }
}

fn add_json_resource_stored<W: std::io::Write + std::io::Seek>(
    writer: &mut zip_writer::RawZipWriter<W>,
    archive_path: &str,
    file_path: &std::path::Path,
) -> Result<(), PackError> {
    let bytes = fs_err::read(file_path)?;
    let normalized = PackageToolJson::from_bytes(bytes).into_bytes();
    zip_writer::add_bytes_stored(writer, archive_path, &normalized)
}

/// JSON normalization performed by `app_packing_tool.jar` for ordinary JSON
/// inputs. The SDK parses them into Fastjson `JSONObject`, whose default
/// backing map is `java.util.HashMap(16)`, and serializes that map without the
/// ordered-field feature. Reproducing HashMap bucket iteration keeps packaged
/// JSON byte-compatible without encoding schema field lists.
struct PackageToolJson {
    bytes: Vec<u8>,
}

impl PackageToolJson {
    fn read(path: &Path) -> Result<Self, PackError> {
        Ok(Self {
            bytes: fs_err::read(path)?,
        })
    }

    fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&self.bytes) else {
            return self.bytes;
        };
        serde_json::to_vec(&Self::into_fastjson_order(value))
            .expect("serializing parsed JSON should not fail")
    }

    fn into_fastjson_order(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(object) => {
                let capacity = Self::java_hash_map_capacity(object.len());
                let mut entries = object.into_iter().collect::<Vec<_>>();
                entries.sort_by_key(|(key, _)| Self::java_hash_bucket(key, capacity));
                let mut ordered = serde_json::Map::with_capacity(entries.len());
                for (key, value) in entries {
                    ordered.insert(key, Self::into_fastjson_order(value));
                }
                serde_json::Value::Object(ordered)
            }
            serde_json::Value::Array(values) => serde_json::Value::Array(
                values.into_iter().map(Self::into_fastjson_order).collect(),
            ),
            value => value,
        }
    }

    fn java_hash_map_capacity(size: usize) -> u32 {
        const MAXIMUM_CAPACITY: u32 = 1 << 30;
        let mut capacity = 16_u32;
        while size > (capacity as usize * 3 / 4) && capacity < MAXIMUM_CAPACITY {
            capacity *= 2;
        }
        capacity
    }

    fn java_hash_bucket(key: &str, capacity: u32) -> u32 {
        let hash = key.encode_utf16().fold(0_u32, |hash, code_unit| {
            hash.wrapping_mul(31).wrapping_add(u32::from(code_unit))
        });
        let spread = hash ^ (hash >> 16);
        spread & (capacity - 1)
    }
}

fn compatible_api_version(version: &str) -> Option<u32> {
    let open = version.rfind('(')?;
    let close = version.rfind(')')?;
    if close <= open {
        return None;
    }
    version[open + 1..close].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_resource_json_orders_share_entries_like_sdk_packing_tool() {
        let value = serde_json::json!({
            "crossAppSharedConfig": [{
                "uri": "arkdown://shared/ProfileCard",
                "allowList": ["com.example.consumer"],
                "srcEntry": "./ets/components/ProfileCard.ets"
            }]
        });

        assert_eq!(
            String::from_utf8(
                PackageToolJson::from_bytes(serde_json::to_vec(&value).unwrap()).into_bytes()
            )
            .unwrap(),
            r#"{"crossAppSharedConfig":[{"srcEntry":"./ets/components/ProfileCard.ets","uri":"arkdown://shared/ProfileCard","allowList":["com.example.consumer"]}]}"#
        );
    }

    #[test]
    fn package_tool_json_matches_fastjson_hash_map_order_for_pkg_context() {
        let input = br#"{"bc-har-dep":{"packageName":"bc-har-dep","bundleName":"","moduleName":"","version":"1.0.0","entryPath":"","isSO":false,"dependencyAlias":""},"entry":{"packageName":"entry","bundleName":"","moduleName":"","version":"","entryPath":"src/main/","isSO":false,"dependencyAlias":""}}"#;
        let output = PackageToolJson::from_bytes(input.as_slice()).into_bytes();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            r#"{"entry":{"isSO":false,"moduleName":"","bundleName":"","dependencyAlias":"","packageName":"entry","entryPath":"src/main/","version":""},"bc-har-dep":{"isSO":false,"moduleName":"","bundleName":"","dependencyAlias":"","packageName":"bc-har-dep","entryPath":"","version":"1.0.0"}}"#
        );
    }

    #[test]
    fn hap_module_json_orders_app_startup_like_package_hap() {
        let value = serde_json::json!({
            "module": {
                "packageName": "entry",
                "name": "entry",
                "appStartup": "$profile:startup_config",
                "descriptionId": 16_777_222,
                "pages": "$profile:main_pages",
                "deviceTypes": ["phone"]
            }
        });

        assert_eq!(
            String::from_utf8(
                PackageToolJson::from_bytes(serde_json::to_vec(&value).unwrap()).into_bytes()
            )
            .unwrap(),
            r#"{"module":{"descriptionId":16777222,"pages":"$profile:main_pages","deviceTypes":["phone"],"appStartup":"$profile:startup_config","name":"entry","packageName":"entry"}}"#
        );
    }

    #[test]
    fn hap_module_json_matches_sdk_fastjson_order_for_component_v2_entry() {
        // Observed from the current DevEco SDK app_packing_tool.jar (fastjson
        // 2.0.57) while packaging component-v2-lifecycle-hap. The package
        // intermediate remains schema ordered; only the archive entry passes
        // through the tool's HashMap-backed JSON serializer.
        let input = br#"{"app":{"bundleName":"com.example.arkdown.componentv2","vendor":"example","versionCode":1000000,"versionName":"1.0.0","icon":"$media:app_icon","label":"$string:app_name","apiReleaseType":"Release","compileSdkVersion":"6.1.1.125","targetAPIVersion":50005017,"minAPIVersion":50005017,"compileSdkType":"HarmonyOS","targetMinorAPIVersion":0,"targetPatchAPIVersion":0,"appEnvironments":[],"bundleType":"app","buildMode":"debug","debug":true,"iconId":16777217,"labelId":16777216},"module":{"name":"entry","type":"entry","mainElement":"EntryAbility","deviceTypes":["phone"],"deliveryWithInstall":true,"installationFree":false,"pages":"$profile:main_pages","abilities":[{"name":"EntryAbility","srcEntry":"./ets/entryability/EntryAbility.ets","exported":true,"startWindowIcon":"$media:start_icon","startWindowBackground":"$color:start_window_background","startWindowIconId":16777219,"startWindowBackgroundId":16777218}],"packageName":"entry","virtualMachine":"ark12.0.6.0","compileMode":"esmodule","dependencies":[]}}"#;

        assert_eq!(
            String::from_utf8(PackageToolJson::from_bytes(input.as_slice()).into_bytes()).unwrap(),
            r#"{"app":{"iconId":16777217,"debug":true,"targetPatchAPIVersion":0,"minAPIVersion":50005017,"icon":"$media:app_icon","buildMode":"debug","label":"$string:app_name","bundleType":"app","versionName":"1.0.0","versionCode":1000000,"appEnvironments":[],"compileSdkType":"HarmonyOS","labelId":16777216,"compileSdkVersion":"6.1.1.125","targetAPIVersion":50005017,"vendor":"example","bundleName":"com.example.arkdown.componentv2","apiReleaseType":"Release","targetMinorAPIVersion":0},"module":{"abilities":[{"exported":true,"startWindowIconId":16777219,"srcEntry":"./ets/entryability/EntryAbility.ets","startWindowBackground":"$color:start_window_background","name":"EntryAbility","startWindowIcon":"$media:start_icon","startWindowBackgroundId":16777218}],"virtualMachine":"ark12.0.6.0","mainElement":"EntryAbility","deviceTypes":["phone"],"pages":"$profile:main_pages","installationFree":false,"name":"entry","deliveryWithInstall":true,"packageName":"entry","compileMode":"esmodule","type":"entry","dependencies":[]}}"#
        );
    }

    #[test]
    fn native_lib_archive_path_preserves_relative_path_after_abi() {
        let path = Path::new("/tmp/pkg/libs/arm64-v8a/plugin/libnative.so");
        assert_eq!(
            ArchivePath::native_lib(path),
            "libs/arm64-v8a/plugin/libnative.so"
        );
    }
}
