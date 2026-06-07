use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::ArkdownConfig;

use crate::error::PackError;
use crate::integrated_hsp::IntegratedHspEntry;
use crate::zip_writer;

fn minify_hap_module_json(content: &str) -> Result<String, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(content)?;
    serde_json::to_string(&order_hap_module_json(value))
}

pub fn minify_package_intermediate_module_json(content: &str) -> Result<String, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(content)?;
    serde_json::to_string(&order_package_module_json(value))
}

fn order_hap_module_json(value: serde_json::Value) -> serde_json::Value {
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
                    "iconId",
                    "debug",
                    "targetPatchAPIVersion",
                    "minAPIVersion",
                    "icon",
                    "buildMode",
                    "label",
                    "bundleType",
                    "versionName",
                    "versionCode",
                    "appEnvironments",
                    "compileSdkType",
                    "labelId",
                    "compileSdkVersion",
                    "targetAPIVersion",
                    "vendor",
                    "bundleName",
                    "apiReleaseType",
                    "targetMinorAPIVersion",
                ],
            ),
        );
    }
    if let Some(module) = root.get("module") {
        out.insert("module".to_string(), order_hap_module(module));
    }
    for (key, value) in root {
        if !out.contains_key(key) {
            out.insert(key.clone(), value.clone());
        }
    }
    serde_json::Value::Object(out)
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

fn order_hap_module(module: &serde_json::Value) -> serde_json::Value {
    let Some(object) = module.as_object() else {
        return module.clone();
    };
    let mut out = serde_json::Map::new();
    for key in [
        "virtualMachine",
        "mainElement",
        "installationFree",
        "deliveryWithInstall",
        "description",
        "compileMode",
        "type",
        "dependencies",
    ] {
        if let Some(value) = object.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    if let Some(abilities) = object.get("abilities") {
        out.insert("abilities".to_string(), order_hap_abilities(abilities));
    }
    for key in [
        "deviceTypes",
        "pages",
        "descriptionId",
        "name",
        "packageName",
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

fn order_module(module: &serde_json::Value) -> serde_json::Value {
    let Some(object) = module.as_object() else {
        return module.clone();
    };
    let mut out = serde_json::Map::new();
    for key in [
        "name",
        "type",
        "description",
        "mainElement",
        "deviceTypes",
        "deliveryWithInstall",
        "installationFree",
        "pages",
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

fn order_hap_abilities(abilities: &serde_json::Value) -> serde_json::Value {
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
                        "exported",
                        "iconId",
                        "startWindowIconId",
                        "icon",
                        "startWindowIcon",
                        "startWindowBackgroundId",
                        "description",
                        "label",
                        "skills",
                        "srcEntry",
                        "descriptionId",
                        "labelId",
                        "startWindowBackground",
                        "name",
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
}

/// A single ability entry included in `pack.info` under `modules[].abilities`.
pub struct PackAbility {
    pub name: String,
    pub label: Option<String>,
}

pub struct PackInput {
    pub module_name: String,
    /// Path to `modules.abc` produced by arkdown-abc.
    pub abc_path: PathBuf,
    /// Path to `resources.index` produced by arkdown-resource.
    pub resources_index: PathBuf,
    /// Path to the processed `module.json` produced by arkdown-profile.
    pub module_json: PathBuf,
    /// Optional native `.so` libraries to bundle.
    pub native_libs: Vec<PathBuf>,
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
    /// Bundle type: `"app"` for HAP, `"shared"` for HSP.
    pub bundle_type: String,
    /// Path to `sourceMaps.map` produced by the bundler; bundled as `ets/sourceMaps.map`.
    pub source_maps_path: Option<PathBuf>,
    /// Abilities list for inclusion in `pack.info` under `modules[].abilities`.
    pub abilities: Vec<PackAbility>,
    /// Remote HSPs to embed inside this HAP (integrated HSP feature, API >= 11).
    ///
    /// Each entry is stored at `libs/<hsp_dir_name>/<hsp_file_name>` inside the archive.
    /// Pass an empty vec for HAPs without integrated HSPs (the common case).
    pub integrated_hsps: Vec<IntegratedHspEntry>,
    pub privacy_profile_path: Option<std::path::PathBuf>,
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
    /// hvigor writes `processDeclarationEntry()` output to the HAR intermediates
    /// directory and then includes the generated stubs in the final archive so
    /// downstream consumers can resolve type information without full tsc.
    /// When `None` (or the directory does not exist), declaration stubs are omitted.
    pub declarations_dir: Option<PathBuf>,
    /// Path to the consumer obfuscation rules file (`consumer-rules.txt`).
    ///
    /// hvigor calls `generateConsumerObConfigFile()` and includes the result so
    /// consumers know which symbols the HAR exposes and how to configure their
    /// own obfuscation for packages that depend on this HAR.
    /// When `None` (or the file does not exist), consumer rules are omitted.
    pub consumer_rules_path: Option<PathBuf>,
}

impl Packager {
    pub fn new(config: Arc<ArkdownConfig>) -> Self {
        Packager { config }
    }

    /// Package a HAP archive.
    ///
    /// Layout:
    /// ```text
    /// module.json           (deflated)
    /// resources.index       (stored)
    /// ets/modules.abc       (stored, API >= 9)  or  modules.abc  (stored, API < 9)
    /// libs/arm64-v8a/*.so   (stored, optional)
    /// ```

    fn generate_pack_info(&self, input: &PackInput) -> String {
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

        let main_ability = input
            .abilities
            .first()
            .map(|a| a.name.as_str())
            .unwrap_or("EntryAbility");

        // HSP uses bundle_type "shared"; all other module types use "entry" as the
        // distro moduleType (HAP entry modules) unless extended in future.
        let module_type = if input.bundle_type == "shared" {
            "shared"
        } else {
            "entry"
        };

        serde_json::json!({
            "summary": {
                "app": {
                    "bundleName": self.config.product.bundle_name,
                    "bundleType": input.bundle_type,
                    "version": {
                        "code": input.bundle_version_code,
                        "name": input.bundle_version_name
                    }
                },
                "modules": [
                    {
                        "mainAbility": main_ability,
                        "deviceType": if input.device_types.is_empty() { vec!["phone".to_string()] } else { input.device_types.clone() },
                        "abilities": abilities_json,
                        "distro": {
                            "moduleType": module_type,
                            "installationFree": input.installation_free,
                            "deliveryWithInstall": input.delivery_with_install,
                            "moduleName": input.module_name
                        },
                        "apiVersion": {
                            "compatible": self.config.sdk_config.compile_api_version,
                            "releaseType": "Release",
                            "target": self.config.sdk_config.compile_api_version
                        }
                    }
                ]
            },
            "packages": [
                {
                    "deviceType": if input.device_types.is_empty() { vec!["phone".to_string()] } else { input.device_types.clone() },
                    "moduleType": module_type,
                    "deliveryWithInstall": input.delivery_with_install,
                    "name": format!("{}-default", input.module_name)
                }
            ]
        })
        .to_string()
    }

    pub fn pack_hap(&self, input: PackInput) -> Result<PathBuf, PackError> {
        if let Some(parent) = input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut writer = zip_writer::create_zip(&input.output_path)?;

        // module.json — deflate compressed, minified as in hvigor PackageHap output.
        if input.module_json.exists() {
            let module_json = fs_err::read_to_string(&input.module_json)?;
            let module_json = minify_hap_module_json(&module_json).unwrap_or(module_json);
            zip_writer::add_bytes_deflate(&mut writer, "module.json", module_json.as_bytes())?;
        } else {
            return Err(PackError::MissingInput("module.json".to_string()));
        }

        // resources.index — stored (binary resource table)
        if input.resources_index.exists() {
            zip_writer::add_file_stored(&mut writer, "resources.index", &input.resources_index)?;
        } else {
            return Err(PackError::MissingInput("resources.index".to_string()));
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
                        add_json_resource_deflate(&mut writer, &dest, entry.path())?;
                    } else if matches!(ext, "json5" | "xml" | "txt" | "html") {
                        zip_writer::add_file_deflate(&mut writer, &dest, entry.path())?;
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

        // ets/sourceMaps.map — deflated; only included if the bundler produced it
        if let Some(ref sm_path) = input.source_maps_path {
            if sm_path.exists() {
                zip_writer::add_file_deflate(&mut writer, "ets/sourceMaps.map", sm_path)?;
            }
        }

        // Additional ETS-side assets copied by CompileArkTS/loader, such as
        // `src/main/ets/common/**/*.json`, are packaged under `ets/`.
        if let Some(ets_dir) = input.abc_path.parent() {
            Self::add_loader_out_ets_assets(&mut writer, ets_dir)?;
        }

        // Native .so libraries — stored, placed under libs/arm64-v8a/
        for so_path in &input.native_libs {
            if so_path.exists() {
                let filename = so_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown.so");
                let dest = ArchivePath::native_lib(filename);
                zip_writer::add_file_stored(&mut writer, &dest, so_path)?;
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

        // module.privacy.json — PAC/privacy profile (deflated), written when
        // `ProcessModulePrivacyProfile` produced a processed privacy profile.
        // Hvigor accepts `pac.json5`; the archive entry remains JSON.
        if let Some(ref privacy_path) = input.privacy_profile_path {
            if privacy_path.exists() {
                if let Ok(content) = std::fs::read_to_string(privacy_path) {
                    if let Ok(value) = json5::from_str::<serde_json::Value>(&content) {
                        let bytes = serde_json::to_vec(&value)
                            .expect("serializing parsed privacy JSON should not fail");
                        zip_writer::add_bytes_deflate(&mut writer, "module.privacy.json", &bytes)?;
                    } else {
                        tracing::warn!(
                            path = %privacy_path.display(),
                            "ProcessModulePrivacyProfile: invalid JSON5 in privacy profile; skipping"
                        );
                    }
                }
            }
        }

        // pack.info — bundle metadata (deflated JSON), written last by hvigor
        // for a normal HAP.
        let mut pack_info = self.generate_pack_info(&input);
        pack_info.push('\n');
        zip_writer::add_bytes_deflate(&mut writer, "pack.info", pack_info.as_bytes())?;

        writer.finish()?;
        Ok(input.output_path)
    }

    /// Package an HSP archive.
    ///
    /// HSP (Harmony Shared Package) has the same on-disk layout as HAP; the
    /// `module.type` field in `module.json` distinguishes them at runtime.
    pub fn pack_hsp(&self, mut input: PackInput) -> Result<PathBuf, PackError> {
        input.bundle_type = "shared".to_string();
        self.pack_hap(input)
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
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let file_name = entry
                    .path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if file_name.ends_with(".d.ts") || file_name.ends_with(".d.ets") {
                    fs_err::copy(entry.path(), cleaned_ets.join(file_name))?;
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
                if matches!(ext, "ets" | "ts")
                    && !path.to_string_lossy().ends_with(".d.ts")
                    && !path.to_string_lossy().ends_with(".d.ets")
                {
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

    /// Package a source HAR archive.
    ///
    /// Layout:
    /// ```text
    /// oh-package.json5          (deflated, optional)
    /// src/main/ets/**           (deflated, source ETS files)
    /// ```
    pub fn pack_har(&self, input: HarPackInput) -> Result<PathBuf, PackError> {
        if let Some(parent) = input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut writer = zip_writer::create_zip(&input.output_path)?;

        // oh-package.json5 — deflate.
        //
        // When declaration stubs are present, add `"types": "index.d.ts"` to
        // the package manifest so downstream consumers can locate declarations
        // without guessing the path (mirrors hvigor processDeclarationEntry).
        if input.oh_package_json.exists() {
            let has_declarations = input.declarations_dir.as_ref().map_or(false, |dir| {
                dir.exists() && dir.join("index.d.ts").is_file()
            });
            if has_declarations {
                // Read, parse (json5), add types field, and write modified JSON.
                let modified =
                    Self::inject_types_field(&input.oh_package_json).unwrap_or_else(|_| {
                        // Fall back to the original file on any error.
                        std::fs::read(input.oh_package_json.as_path()).unwrap_or_default()
                    });
                zip_writer::add_bytes_deflate(&mut writer, "oh-package.json5", &modified)?;
            } else {
                zip_writer::add_file_deflate(
                    &mut writer,
                    "oh-package.json5",
                    &input.oh_package_json,
                )?;
            }
        }

        // Source .ets files from source_dir/
        if input.source_dir.exists() {
            for entry in walkdir::WalkDir::new(&input.source_dir)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let rel = entry
                    .path()
                    .strip_prefix(&input.source_dir)
                    .unwrap_or(entry.path());
                let dest = ArchivePath::har_ets(rel);
                zip_writer::add_file_deflate(&mut writer, &dest, entry.path())?;
            }
        }

        // src/main/resources/ — embed resources in the source HAR so consumers
        // can use the HAR's resource entries. Matches hvigor's HAR layout.
        if input.resources_dir.exists() {
            for entry in walkdir::WalkDir::new(&input.resources_dir)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let rel = entry
                    .path()
                    .strip_prefix(&input.resources_dir)
                    .unwrap_or(entry.path());
                let dest = ArchivePath::har_resources(rel);
                zip_writer::add_file_deflate(&mut writer, &dest, entry.path())?;
            }
        }

        // src/main/resource/ — permission reason string resources for HSP/HAR.
        if let Some(resource_str_dir) = input.resource_str_dir.as_ref() {
            if resource_str_dir.exists() {
                for entry in walkdir::WalkDir::new(resource_str_dir)
                    .min_depth(1)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                {
                    let rel = entry
                        .path()
                        .strip_prefix(resource_str_dir)
                        .unwrap_or(entry.path());
                    let dest = ArchivePath::har_resource_str(rel);
                    zip_writer::add_file_deflate(&mut writer, &dest, entry.path())?;
                }
            }
        }

        // Declaration stubs (index.d.ts, index.d.ets) — packaged at the HAR root.
        //
        // hvigor's source HAR layout places declaration stubs generated by
        // `processDeclarationEntry()` directly at the archive root so
        // downstream consumers can locate them via `oh-package.json5#types`.
        if let Some(declarations_dir) = input.declarations_dir.as_ref() {
            if declarations_dir.exists() {
                for entry in walkdir::WalkDir::new(declarations_dir)
                    .min_depth(1)
                    .max_depth(1) // only top-level stubs
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                {
                    let file_name = entry
                        .path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();
                    // Include only .d.ts and .d.ets declaration stubs.
                    if file_name.ends_with(".d.ts") || file_name.ends_with(".d.ets") {
                        zip_writer::add_file_deflate(&mut writer, file_name, entry.path())?;
                    }
                }
            }
        }

        // Consumer obfuscation rules (`consumer-rules.txt`) — packaged at the HAR root.
        //
        // hvigor's `generateConsumerObConfigFile()` writes the HAR's obfuscation rules
        // so that downstream consumers can extend their own obfuscation config.
        if let Some(consumer_rules) = input.consumer_rules_path.as_ref() {
            if consumer_rules.exists() {
                zip_writer::add_file_deflate(&mut writer, "consumer-rules.txt", consumer_rules)?;
            }
        }

        writer.finish()?;
        Ok(input.output_path)
    }
}

// Private helpers for HAR assembly
impl Packager {
    /// Read `oh-package.json5`, add/update `"types": "index.d.ts"`, and return the
    /// modified JSON bytes.
    ///
    /// hvigor's `processDeclarationEntry()` writes this field so downstream consumers
    /// can locate the TypeScript/ArkTS declaration stubs without guessing.
    fn inject_types_field(path: &std::path::Path) -> Result<Vec<u8>, PackError> {
        let content = fs_err::read_to_string(path)?;
        let mut pkg: serde_json::Value =
            json5::from_str(&content).map_err(|e| PackError::MalformedPackInfo(e.to_string()))?;
        if let Some(obj) = pkg.as_object_mut() {
            // Only add if not already set; preserve existing value if present.
            obj.entry("types")
                .or_insert_with(|| serde_json::Value::String("index.d.ts".to_string()));
        }
        serde_json::to_vec_pretty(&pkg).map_err(|e| PackError::MalformedPackInfo(e.to_string()))
    }
}

// Private helpers for HAP/HSP assembly
impl Packager {
    fn add_loader_out_ets_assets<W: std::io::Write + std::io::Seek>(
        writer: &mut zip::ZipWriter<W>,
        ets_dir: &std::path::Path,
    ) -> Result<(), PackError> {
        for entry in walkdir::WalkDir::new(ets_dir)
            .min_depth(1)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            let rel = path.strip_prefix(ets_dir).unwrap_or(path);
            let rel_text = rel.to_string_lossy().replace('\\', "/");
            if rel_text == "modules.abc"
                || rel_text == "sourceMaps.map"
                || rel_text.starts_with("cache/")
                || path.extension().and_then(|e| e.to_str()) == Some("js")
            {
                continue;
            }
            let dest = ArchivePath::ets(&rel_text);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "json" {
                add_json_resource_deflate(writer, &dest, path)?;
            } else if matches!(ext, "json5" | "xml" | "txt" | "html") {
                zip_writer::add_file_deflate(writer, &dest, path)?;
            } else {
                zip_writer::add_file_stored(writer, &dest, path)?;
            }
        }
        Ok(())
    }

    fn add_profile_guided_ap<W: std::io::Write + std::io::Seek>(
        writer: &mut zip::ZipWriter<W>,
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

struct ArchivePath;

impl ArchivePath {
    fn resources(rel: &std::path::Path) -> String {
        format!("resources/{}", arkdown_utils::to_unix_path(rel))
    }

    fn native_lib(filename: &str) -> String {
        format!("libs/arm64-v8a/{filename}")
    }

    fn integrated_hsp(dir_name: &str, file_name: &str) -> String {
        format!("libs/{dir_name}/{file_name}")
    }

    fn har_ets(rel: &std::path::Path) -> String {
        format!("src/main/ets/{}", arkdown_utils::to_unix_path(rel))
    }

    fn har_resources(rel: &std::path::Path) -> String {
        format!("src/main/resources/{}", arkdown_utils::to_unix_path(rel))
    }

    fn har_resource_str(rel: &std::path::Path) -> String {
        format!("src/main/resource/{}", arkdown_utils::to_unix_path(rel))
    }

    fn ets(rel_text: &str) -> String {
        format!("ets/{rel_text}")
    }

    fn profile_guided_ap(module_name: &str) -> String {
        format!("ap/{}", Self::profile_guided_ap_file(module_name))
    }

    fn profile_guided_ap_file(module_name: &str) -> String {
        format!("{module_name}.ap")
    }
}

fn add_json_resource_deflate<W: std::io::Write + std::io::Seek>(
    writer: &mut zip::ZipWriter<W>,
    archive_path: &str,
    file_path: &std::path::Path,
) -> Result<(), PackError> {
    let bytes = fs_err::read(file_path)?;
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        let minified = serde_json::to_vec(&value).expect("serializing parsed JSON should not fail");
        zip_writer::add_bytes_deflate(writer, archive_path, &minified)
    } else {
        zip_writer::add_bytes_deflate(writer, archive_path, &bytes)
    }
}
