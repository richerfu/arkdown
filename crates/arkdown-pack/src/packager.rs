use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::ArkdownConfig;

use crate::error::PackError;
use crate::zip_writer;

pub struct Packager {
    pub config: Arc<ArkdownConfig>,
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

        // resources.index — stored (binary resource table)
        if input.resources_index.exists() {
            zip_writer::add_file_stored(
                &mut writer,
                "resources.index",
                &input.resources_index,
            )?;
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
            zip_writer::add_file_deflate(
                &mut writer,
                "oh-package.json5",
                &input.oh_package_json,
            )?;
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
                let dest = format!(
                    "src/main/ets/{}",
                    rel.to_string_lossy().replace('\\', "/")
                );
                zip_writer::add_file_deflate(&mut writer, &dest, entry.path())?;
            }
        }

        writer.finish()?;
        Ok(input.output_path)
    }
}
