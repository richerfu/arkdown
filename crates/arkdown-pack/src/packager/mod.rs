use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::ArkdownConfig;

use crate::error::PackError;
use crate::zip_writer;

mod archive;
mod har;
mod inputs;
mod module_json;

use archive::{
    add_json_resource_stored, compatible_api_version, ArchivePath, PackageToolJson,
    ProfileResourceFileName,
};

pub use inputs::{
    BundledHarResourceInput, HarBundledDependencyInput, HarPackInput, HarPackageParameterInput,
    HarVariantInput, HspInterfaceHarInput, HspTgzInput, PackInfoInput, PackInfoModuleInput,
    PackInput,
};
pub use module_json::minify_package_intermediate_module_json;

pub struct Packager {
    pub config: Arc<ArkdownConfig>,
    compress_level: u32,
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

    fn pack_info_package_name(&self, module_name: &str, target_name: &str) -> String {
        self.config
            .product
            .artifact_name
            .clone()
            .unwrap_or_else(|| format!("{module_name}-{target_name}"))
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
    fn generate_pack_info(&self, input: &PackInfoInput) -> String {
        let compatible_api_version_value =
            compatible_api_version(&self.config.product.compatible_sdk_version)
                .unwrap_or(self.config.sdk_config.compile_api_version);
        let target_api_version = self
            .config
            .product
            .target_sdk_version
            .as_deref()
            .and_then(compatible_api_version)
            .unwrap_or(self.config.sdk_config.compile_api_version);

        let module_type = input
            .module_json
            .pointer("/module/type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("entry");
        let current_device_types = Self::pack_info_device_types(&input.device_types);
        let mut modules = Vec::with_capacity(1 + input.additional_modules.len());
        modules.push(self.pack_info_module(
            &input.module_name,
            &input.module_json,
            &input.resources_src_dirs,
            current_device_types,
            compatible_api_version_value,
            target_api_version,
            Some(input.installation_free),
            Some(input.delivery_with_install),
            &input.bundle_type,
        ));
        modules.extend(input.additional_modules.iter().map(|additional| {
            self.pack_info_module(
                &additional.module_name,
                &additional.module_json,
                &additional.resources_src_dirs,
                Self::pack_info_device_types(&additional.device_types),
                compatible_api_version_value,
                target_api_version,
                None,
                None,
                &input.bundle_type,
            )
        }));

        let mut version = serde_json::Map::new();
        version.insert("code".to_string(), input.bundle_version_code.into());
        version.insert("name".to_string(), input.bundle_version_name.clone().into());
        if let Some(build_version) = input.bundle_build_version.as_ref() {
            version.insert("build".to_string(), build_version.clone().into());
        }
        if let Some(min_compatible_version_code) = input.min_compatible_version_code {
            version.insert(
                "minCompatibleVersionCode".to_string(),
                min_compatible_version_code.into(),
            );
        }

        let bundle_name = if matches!(self.config.output_target, arkdown_core::OutputTarget::Hsp)
            && self
                .config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|options| options.integrated_hsp)
                .unwrap_or(false)
        {
            String::new()
        } else {
            self.config.product.bundle_name.clone()
        };

        let package_device_types = if input.package_device_types.is_empty() {
            Self::pack_info_device_types(&input.device_types)
        } else {
            input.package_device_types.clone()
        };
        let mut package = serde_json::Map::new();
        package.insert("deviceType".to_string(), package_device_types.into());
        package.insert("moduleType".to_string(), module_type.into());
        package.insert(
            "deliveryWithInstall".to_string(),
            input.delivery_with_install.into(),
        );
        package.insert("name".to_string(), input.package_name.clone().into());

        serde_json::json!({
            "summary": {
                "app": {
                    "bundleName": bundle_name,
                    "bundleType": input.bundle_type,
                    "version": serde_json::Value::Object(version)
                },
                "modules": modules
            },
            "packages": [serde_json::Value::Object(package)]
        })
        .to_string()
    }

    #[allow(clippy::too_many_arguments)]
    fn pack_info_module(
        &self,
        module_name: &str,
        module_json: &serde_json::Value,
        resources_src_dirs: &[PathBuf],
        device_types: Vec<String>,
        compatible_api_version: u32,
        target_api_version: u32,
        installation_free_fallback: Option<bool>,
        delivery_with_install_fallback: Option<bool>,
        bundle_type: &str,
    ) -> serde_json::Value {
        let mut module = serde_json::Map::new();
        if let Some(main_ability) = module_json
            .pointer("/module/mainElement")
            .and_then(serde_json::Value::as_str)
        {
            module.insert("mainAbility".to_string(), main_ability.into());
        }
        module.insert("deviceType".to_string(), device_types.into());
        if let Some(abilities) = Self::pack_info_abilities(module_json) {
            module.insert("abilities".to_string(), abilities.into());
        }
        if let Some(extension_abilities) =
            Self::pack_info_extension_abilities(resources_src_dirs, module_json)
        {
            module.insert("extensionAbilities".to_string(), extension_abilities.into());
        }

        let module_type = module_json
            .pointer("/module/type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("entry");
        let installation_free = match bundle_type {
            "atomicService" => Some(true),
            "app" => Some(false),
            _ => module_json
                .pointer("/module/installationFree")
                .and_then(serde_json::Value::as_bool)
                .or(installation_free_fallback),
        };
        let delivery_with_install = module_json
            .pointer("/module/deliveryWithInstall")
            .and_then(serde_json::Value::as_bool)
            .or(delivery_with_install_fallback);
        let mut distro = serde_json::Map::new();
        distro.insert("moduleType".to_string(), module_type.into());
        if let Some(installation_free) = installation_free {
            distro.insert("installationFree".to_string(), installation_free.into());
        }
        if let Some(delivery_with_install) = delivery_with_install {
            distro.insert(
                "deliveryWithInstall".to_string(),
                delivery_with_install.into(),
            );
        }
        distro.insert(
            "moduleName".to_string(),
            module_json
                .pointer("/module/name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(module_name)
                .into(),
        );
        module.insert("distro".to_string(), distro.into());
        module.insert(
            "apiVersion".to_string(),
            serde_json::json!({
                "compatible": compatible_api_version,
                "releaseType": "Release",
                "target": target_api_version
            }),
        );
        serde_json::Value::Object(module)
    }

    fn pack_info_device_types(device_types: &[String]) -> Vec<String> {
        if device_types.is_empty() {
            vec!["phone".to_string()]
        } else {
            device_types.to_vec()
        }
    }

    fn pack_info_abilities(module_json: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
        let abilities = module_json
            .pointer("/module/abilities")
            .and_then(serde_json::Value::as_array)?;
        Some(
            abilities
                .iter()
                .filter_map(|ability| {
                    let ability = ability.as_object()?;
                    let mut output = serde_json::Map::new();
                    for key in ["name", "label", "visible"] {
                        if let Some(value) = ability.get(key) {
                            output.insert(key.to_string(), value.clone());
                        }
                    }
                    if let Some(forms) = ability.get("forms").and_then(serde_json::Value::as_array)
                    {
                        output.insert(
                            "forms".to_string(),
                            serde_json::Value::Array(
                                forms
                                    .iter()
                                    .filter_map(Self::pack_info_form_entry)
                                    .collect(),
                            ),
                        );
                    }
                    (!output.is_empty()).then_some(serde_json::Value::Object(output))
                })
                .collect(),
        )
    }

    fn pack_info_extension_abilities(
        resources_src_dirs: &[PathBuf],
        module_json: &serde_json::Value,
    ) -> Option<Vec<serde_json::Value>> {
        let extension_abilities = module_json
            .pointer("/module/extensionAbilities")
            .and_then(|value| value.as_array())?;

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
                let forms = Self::pack_info_forms(resources_src_dirs, ability_object);
                entry.insert("forms".to_string(), serde_json::Value::Array(forms));
            }
            out.push(serde_json::Value::Object(entry));
        }
        Some(out)
    }

    fn pack_info_forms(
        resources_src_dirs: &[PathBuf],
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
            let Some(profile_path) = resources_src_dirs
                .iter()
                .map(|resources_src_dir| {
                    resources_src_dir
                        .join("base")
                        .join("profile")
                        .join(profile_file.file_name())
                })
                .find(|path| path.is_file())
            else {
                continue;
            };
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
                out.insert(key.to_string(), "JS".into());
            } else if let Some(value) = object.get(key) {
                out.insert(key.to_string(), value.clone());
            }
        }
        (!out.is_empty()).then_some(serde_json::Value::Object(out))
    }

    /// Write the exact `pack.info` consumed by FastPackage and PackageApp.
    pub fn write_pack_info(
        &self,
        input: &PackInfoInput,
        output_path: &Path,
    ) -> Result<PathBuf, PackError> {
        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut pack_info = self.generate_pack_info(input);
        pack_info.push('\n');
        fs_err::write(output_path, pack_info)?;
        Ok(output_path.to_path_buf())
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
        let pkg_sdk_info = if let Some(pkg_sdk_info_path) = &input.pkg_sdk_info_path {
            if !pkg_sdk_info_path.exists() {
                return Err(PackError::MissingInput(format!(
                    "pkgSdkInfo.json ({})",
                    pkg_sdk_info_path.display()
                )));
            }
            Some(fs_err::read(pkg_sdk_info_path)?)
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
                    .follow_links(false)
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

        // Additional ETS-side assets copied by CompileArkTS/loader, such as
        // widget/component_collection.json and widgets.abc, precede the main
        // ABC in SDK app_packing_tool output order.
        if let Some(ets_dir) = input.abc_path.as_deref().and_then(Path::parent) {
            Self::add_loader_out_ets_assets(&mut writer, ets_dir, &input.output_path)?;
        }

        // Current Hvigor `BasePackHapTask.generateCommand` passes the complete
        // loader-out ETS directory through `--ets-path`. A hybrid ArkTS 1.2
        // module therefore packages both the dynamic and static merged ABCs.
        // Keep the compiler-owned primary path first, then the explicitly
        // declared additional artifacts; never infer a stale sibling file.
        let mut abc_paths = Vec::with_capacity(
            usize::from(input.abc_path.is_some()) + input.additional_abc_paths.len(),
        );
        abc_paths.extend(input.abc_path.iter().cloned());
        abc_paths.extend(input.additional_abc_paths.iter().cloned());
        abc_paths.dedup();
        for abc_path in abc_paths {
            if !abc_path.is_file() {
                return Err(PackError::MissingInput(abc_path.display().to_string()));
            }
            let abc_name = abc_path
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| matches!(*name, "modules.abc" | "modules_static.abc"))
                .ok_or_else(|| {
                    PackError::MissingInput(format!(
                        "unsupported merged ABC name ({})",
                        abc_path.display()
                    ))
                })?;
            let abc_dest = if self.config.sdk_config.compile_api_version >= 9 {
                format!("ets/{abc_name}")
            } else {
                abc_name.to_string()
            };
            zip_writer::add_file_stored(&mut writer, &abc_dest, &abc_path)?;
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

        if input
            .aot_mode
            .as_deref()
            .is_some_and(|mode| !mode.is_empty())
        {
            Self::add_aot_outputs(&mut writer, &input)?;
            Self::add_profile_guided_ap(&mut writer, &input)?;
        }

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
        let pack_info_input = PackInfoInput {
            module_name: input.module_name.clone(),
            package_name: self.pack_info_package_name(&input.module_name, &input.target_name),
            module_json: pack_info_module_json_value
                .as_ref()
                .or(module_json_value.as_ref())
                .cloned()
                .ok_or_else(|| {
                    PackError::MalformedPackInfo("module.json is missing".to_string())
                })?,
            resources_src_dirs: input.resources_src_dir.clone().into_iter().collect(),
            bundle_version_code: input.bundle_version_code,
            bundle_version_name: input.bundle_version_name.clone(),
            bundle_build_version: input.bundle_build_version.clone(),
            min_compatible_version_code: pack_info_module_json_value
                .as_ref()
                .or(module_json_value.as_ref())
                .and_then(|value| value.pointer("/app/minCompatibleVersionCode"))
                .and_then(serde_json::Value::as_u64),
            bundle_type: input.bundle_type.clone(),
            additional_modules: Vec::new(),
            device_types: input.device_types.clone(),
            package_device_types: Vec::new(),
            installation_free: input.installation_free,
            delivery_with_install: input.delivery_with_install,
        };
        let pack_info = if let Some(path) = input.pack_info_path.as_ref() {
            fs_err::read(path)?
        } else {
            let mut pack_info = self.generate_pack_info(&pack_info_input);
            pack_info.push('\n');
            pack_info.into_bytes()
        };
        zip_writer::add_bytes_stored(&mut writer, "pack.info", &pack_info)?;

        // The SDK package tool writes pkgContextInfo after its regular HAP
        // entries (including pack.info), even though the CLI option is supplied
        // alongside the other input paths.
        if let Some(pkg_context) = pkg_context {
            zip_writer::add_bytes_stored(&mut writer, "pkgContextInfo.json", &pkg_context)?;
        }
        if let Some(pkg_sdk_info) = pkg_sdk_info {
            zip_writer::add_bytes_stored(&mut writer, "pkgSdkInfo.json", &pkg_sdk_info)?;
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
