use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::ArkdownConfig;

use crate::error::PackError;
use crate::zip_writer;

fn generate_pack_info(config: &ArkdownConfig, input: &PackInput) -> String {
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

    serde_json::json!({
        "summary": {
            "app": {
                "bundleName": config.product.bundle_name,
                "bundleType": input.bundle_type,
                "version": {
                    "code": input.bundle_version_code,
                    "name": input.bundle_version_name
                }
            },
            "modules": [
                {
                    "mainAbility": main_ability,
                    "deviceType": ["phone"],
                    "abilities": abilities_json,
                    "distro": {
                        "moduleType": "entry",
                        "moduleName": input.module_name,
                        "installationFree": false,
                        "deliveryWithInstall": true
                    },
                    "apiVersion": {
                        "compatible": config.sdk_config.compile_api_version,
                        "releaseType": "Release",
                        "target": config.sdk_config.compile_api_version
                    }
                }
            ]
        },
        "packages": [
            {
                "deviceType": ["phone"],
                "moduleType": "entry",
                "deliveryWithInstall": true,
                "name": format!("{}-default", input.module_name)
            }
        ]
    })
    .to_string()
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
}

pub struct HarPackInput {
    pub module_name: String,
    /// Source ETS files directory (`src/main/ets/`).
    pub source_dir: PathBuf,
    /// Resources directory (`src/main/resources/`).
    pub resources_dir: PathBuf,
    /// Path to `oh-package.json5`.
    pub oh_package_json: PathBuf,
    /// Destination archive path.
    pub output_path: PathBuf,
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
    pub fn pack_hap(&self, input: PackInput) -> Result<PathBuf, PackError> {
        if let Some(parent) = input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut writer = zip_writer::create_zip(&input.output_path)?;

        // module.json — deflate compressed
        if input.module_json.exists() {
            zip_writer::add_file_deflate(&mut writer, "module.json", &input.module_json)?;
        } else {
            return Err(PackError::MissingInput("module.json".to_string()));
        }

        // pack.info — bundle metadata (deflated JSON)
        let pack_info = generate_pack_info(&self.config, &input);
        zip_writer::add_bytes_deflate(&mut writer, "pack.info", pack_info.as_bytes())?;

        // .pages.info — binary router-map page index (hvigorw format)
        zip_writer::add_bytes_deflate(&mut writer, ".pages.info", &[0x20, 0x22, 0, 0, 0, 0, 0, 0])?;

        // resources.index — stored (binary resource table)
        if input.resources_index.exists() {
            zip_writer::add_file_stored(&mut writer, "resources.index", &input.resources_index)?;
        } else {
            return Err(PackError::MissingInput("resources.index".to_string()));
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

        // Resource files — copy from src/main/resources/ into resources/ in the HAP
        if let Some(res_src) = &input.resources_src_dir {
            if res_src.exists() {
                for entry in walkdir::WalkDir::new(res_src)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                {
                    let rel = entry.path().strip_prefix(res_src).unwrap_or(entry.path());
                    let dest = format!("resources/{}", rel.to_string_lossy().replace('\\', "/"));
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

        // Native .so libraries — stored, placed under libs/arm64-v8a/
        for so_path in &input.native_libs {
            if so_path.exists() {
                let arch_dir = "libs/arm64-v8a";
                let filename = so_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown.so");
                let dest = format!("{}/{}", arch_dir, filename);
                zip_writer::add_file_stored(&mut writer, &dest, so_path)?;
            }
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

        // oh-package.json5 — deflate
        if input.oh_package_json.exists() {
            zip_writer::add_file_deflate(&mut writer, "oh-package.json5", &input.oh_package_json)?;
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
                let dest = format!("src/main/ets/{}", rel.to_string_lossy().replace('\\', "/"));
                zip_writer::add_file_deflate(&mut writer, &dest, entry.path())?;
            }
        }

        writer.finish()?;
        Ok(input.output_path)
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
