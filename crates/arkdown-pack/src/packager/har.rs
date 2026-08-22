//! HAR/HSP-interface packaging implementation.
//!
//! This module owns the HAR artifact shapes (source, obfuscated, bytecode,
//! and HSP interface HAR) plus the package-manager-specific assembly helpers.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_core::{ArkdownConfig, HarArtifactType};
use arkdown_path::{AbsPath, ArtifactPath, ArtifactPathSerializer};
use indexmap::IndexMap;

use crate::error::PackError;

use super::*;

impl Packager {
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

        // `PrepareSharedHarRes` is intentionally OHPM-only even when the
        // project-wide package-manager decision selected npm: it reads
        // `getOhPackageJson5Path()` and always writes `oh-package.json5`.
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
        let mut entries = self.har_variant_entries(&input, &variant)?;
        if let Some(package_cache_dir) = variant.package_cache_dir.as_ref() {
            entries.write_directory(package_cache_dir)?;
        }
        entries.write_archive()?;

        Ok(input.output_path)
    }

    /// Materialize the exact temporary package tree owned by hvigor's
    /// `ProcessHarArtifacts` task without creating the final `.har` archive.
    /// Keeping this boundary observable is required for direct task execution
    /// and for plugins that attach lifecycle hooks to `ProcessHarArtifacts`.
    pub fn process_har_artifacts(
        &self,
        input: &HarPackInput,
        variant: &HarVariantInput,
        package_cache_dir: &Path,
    ) -> Result<PathBuf, PackError> {
        let mut entries = self.har_variant_entries(input, variant)?;
        entries.write_directory(package_cache_dir)?;
        Ok(package_cache_dir.to_path_buf())
    }

    /// `PackageHar` invokes `ohpm pack` over the directory produced by
    /// `ProcessHarArtifacts`. Re-materializing from that directory preserves
    /// the task boundary and makes a direct `ProcessHarArtifacts` invocation
    /// stop before an archive is emitted.
    pub fn pack_har_artifacts(
        &self,
        input: &HarPackInput,
        package_cache_dir: &Path,
        npm_executable: Option<&Path>,
    ) -> Result<PathBuf, PackError> {
        HarArtifactPackRunner::new(input, package_cache_dir, npm_executable).execute()
    }

    fn har_variant_entries<'a>(
        &self,
        input: &'a HarPackInput,
        variant: &HarVariantInput,
    ) -> Result<HarArchiveEntries<'a>, PackError> {
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

        entries.insert_bytes(input.manifest_archive_name(), manifest.serialize()?);
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
        Ok(entries)
    }

    /// Stage the directory shape consumed by hvigor `PackageSignHar`.
    ///
    /// `PackageHar` owns a flat package cache. `PackageSignHar` copies that
    /// cache below a fresh `package/` directory and passes its parent to
    /// `sdk-sign-tool-full.jar`; keeping this copy separate ensures signing
    /// never mutates the unsigned HAR inputs.
    pub fn stage_har_signing_directory(
        &self,
        package_cache_dir: &Path,
        signing_root: &Path,
    ) -> Result<PathBuf, PackError> {
        if !package_cache_dir.is_dir() {
            return Err(PackError::MissingInput(format!(
                "HAR package cache does not exist: {}",
                package_cache_dir.display()
            )));
        }
        if signing_root.exists() {
            if !signing_root.is_dir() {
                return Err(PackError::MissingInput(format!(
                    "HAR signing temp path is not a directory: {}",
                    signing_root.display()
                )));
            }
            fs_err::remove_dir_all(signing_root)?;
        }
        fs_err::create_dir_all(signing_root)?;

        let package_root = signing_root.join("package");
        let mut entries = HarArchiveEntries::new(signing_root);
        entries.collect_materialized_tree(package_cache_dir, Path::new(""))?;
        entries.write_directory(&package_root)?;
        Ok(signing_root.to_path_buf())
    }

    /// Repack a signed HAR task directory exactly like
    /// `execOhpmPack(taskTmpRoot, outputFile, ..., false)`: entries already
    /// include the `package/` directory, so no additional prefix is applied.
    pub fn pack_signed_har_directory(
        &self,
        signing_root: &Path,
        output_path: &Path,
    ) -> Result<PathBuf, PackError> {
        if !signing_root.is_dir() {
            return Err(PackError::MissingInput(format!(
                "HAR signing directory does not exist: {}",
                signing_root.display()
            )));
        }
        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let mut entries = HarArchiveEntries::new(output_path);
        entries.collect_materialized_tree(signing_root, Path::new(""))?;
        entries.write_archive_with_prefix(None)?;
        Ok(output_path.to_path_buf())
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

/// Executes the package-manager-specific `PackageHar` boundary.
///
/// `AbstractPackageHar.doTaskAction` uses the internal OHPM tar writer for an
/// OHPM project, but invokes the npm launcher beside `process.execPath` for a
/// legacy npm project. Keeping that branch here prevents an npm project from
/// being silently emitted as an OHPM-style `.har` file.
struct HarArtifactPackRunner<'a> {
    input: &'a HarPackInput,
    package_cache_dir: &'a Path,
    npm_executable: Option<&'a Path>,
}

impl<'a> HarArtifactPackRunner<'a> {
    fn new(
        input: &'a HarPackInput,
        package_cache_dir: &'a Path,
        npm_executable: Option<&'a Path>,
    ) -> Self {
        Self {
            input,
            package_cache_dir,
            npm_executable,
        }
    }

    fn execute(&self) -> Result<PathBuf, PackError> {
        if !self.package_cache_dir.is_dir() {
            return Err(PackError::MissingInput(format!(
                "HAR package cache does not exist: {}",
                self.package_cache_dir.display()
            )));
        }
        self.reset_output_dir()?;
        if self.input.is_ohpm_project() {
            return self.execute_ohpm_pack();
        }
        self.execute_npm_pack()
    }

    fn output_dir(&self) -> Result<&Path, PackError> {
        self.input
            .output_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| {
                PackError::MissingInput(format!(
                    "HAR output path has no parent directory: {}",
                    self.input.output_path.display()
                ))
            })
    }

    fn reset_output_dir(&self) -> Result<(), PackError> {
        let output_dir = self.output_dir()?;
        if output_dir.exists() {
            if !output_dir.is_dir() {
                return Err(PackError::MissingInput(format!(
                    "HAR output directory is not a directory: {}",
                    output_dir.display()
                )));
            }
            fs_err::remove_dir_all(output_dir)?;
        }
        fs_err::create_dir_all(output_dir)?;
        Ok(())
    }

    fn execute_ohpm_pack(&self) -> Result<PathBuf, PackError> {
        let mut entries = HarArchiveEntries::new(&self.input.output_path);
        entries.collect_materialized_tree(self.package_cache_dir, Path::new(""))?;
        entries.write_archive()?;
        Ok(self.input.output_path.clone())
    }

    fn execute_npm_pack(&self) -> Result<PathBuf, PackError> {
        let npm_executable = self.npm_executable.ok_or_else(|| {
            PackError::MissingNpmLauncher(
                "the Node runtime did not provide its adjacent npm launcher".to_string(),
            )
        })?;
        if Self::has_explicit_parent(npm_executable) && !npm_executable.is_file() {
            return Err(PackError::MissingNpmLauncher(
                npm_executable.display().to_string(),
            ));
        }

        let output = self
            .npm_command(npm_executable)
            .arg("pack")
            .current_dir(self.package_cache_dir)
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            return Err(PackError::NpmPackFailed {
                status: output.status.code(),
                stdout,
                stderr,
            });
        }

        let archive_name = stdout
            .lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty())
            .ok_or_else(|| PackError::InvalidNpmPackOutput(stdout.clone()))?;
        let archive_path = Path::new(archive_name);
        if archive_path.file_name() != Some(archive_path.as_os_str())
            || archive_path.extension() != Some(std::ffi::OsStr::new("tgz"))
        {
            return Err(PackError::InvalidNpmPackOutput(stdout));
        }

        let source = self.package_cache_dir.join(archive_path);
        if !source.is_file() {
            return Err(PackError::InvalidNpmPackOutput(format!(
                "npm reported '{archive_name}', but {} was not created",
                source.display()
            )));
        }
        let destination = self.output_dir()?.join(archive_path);
        fs_err::copy(source, &destination)?;
        Ok(destination)
    }

    fn has_explicit_parent(path: &Path) -> bool {
        path.is_absolute()
            || path
                .parent()
                .is_some_and(|parent| !parent.as_os_str().is_empty())
    }

    #[cfg(not(windows))]
    fn npm_command(&self, npm_executable: &Path) -> Command {
        Command::new(npm_executable)
    }

    #[cfg(windows)]
    fn npm_command(&self, npm_executable: &Path) -> Command {
        let command_processor = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        let mut command = Command::new(command_processor);
        command.args(["/D", "/S", "/C"]).arg(npm_executable);
        command
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
        let mode = config.build_mode.as_str();
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
            if input.is_ohpm_project() {
                package.insert(
                    "artifactType".to_string(),
                    serde_json::Value::String("obfuscation".to_string()),
                );
            } else {
                let ohos = package
                    .entry("ohos")
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                    .as_object_mut()
                    .ok_or_else(|| {
                        PackError::MalformedPackInfo(
                            "package.json ohos field must be an object".into(),
                        )
                    })?;
                ohos.insert(
                    "artifactType".to_string(),
                    serde_json::Value::String("obfuscation".to_string()),
                );
            }
        }

        if input.is_ohpm_project() {
            Self::apply_workers(package, variant)?;
            Self::apply_runtime_only(package, variant)?;
        }
        if matches!(variant.artifact_type, HarArtifactType::Bytecode) {
            Self::metadata(package)?
                .insert("byteCodeHar".to_string(), serde_json::Value::Bool(true));
            // JavaScript `delete package.main` preserves all remaining key
            // positions. `shift_remove` provides the same IndexMap behavior.
            package.shift_remove("main");
        }
        if input.is_ohpm_project() {
            Self::apply_resource_directories(package, variant)?;
        }
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

    fn collect_materialized_tree(
        &mut self,
        root: &Path,
        destination: &Path,
    ) -> Result<(), PackError> {
        if !root.is_dir() {
            return Err(PackError::MissingInput(format!(
                "required HAR directory does not exist: {}",
                root.display()
            )));
        }
        for entry in walkdir::WalkDir::new(root).min_depth(1) {
            let entry = entry?;
            let relative = entry.path().strip_prefix(root).map_err(|error| {
                PackError::MissingInput(format!("invalid HAR directory path: {error}"))
            })?;
            let archive_path = destination.join(relative);
            let archive_path = ArtifactPathSerializer::rel_to_slash(&archive_path);
            if entry.file_type().is_dir() {
                self.directories.insert(archive_path);
            } else if entry.file_type().is_file() {
                self.files.insert(archive_path, entry.into_path());
            }
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
    fn write_archive(self) -> Result<(), PackError> {
        self.write_archive_with_prefix(Some("package"))
    }

    fn write_archive_with_prefix(mut self, prefix: Option<&str>) -> Result<(), PackError> {
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
                    self.archive_member_path(prefix, archive_path, true),
                    std::io::empty(),
                )?;
                continue;
            }
            if let Some(bytes) = self.inline_files.get(archive_path) {
                let mut header = Self::file_header(bytes.len() as u64);
                archive.append_data(
                    &mut header,
                    self.archive_member_path(prefix, archive_path, false),
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
            archive.append_data(
                &mut header,
                self.archive_member_path(prefix, archive_path, false),
                &mut input,
            )?;
        }
        archive.finish()?;
        archive.into_inner()?.finish()?;
        Ok(())
    }

    fn archive_member_path(&self, prefix: Option<&str>, path: &str, directory: bool) -> String {
        let mut member = prefix.map_or_else(String::new, |prefix| format!("{prefix}/"));
        member.push_str(path);
        if directory {
            member.push('/');
        }
        member
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
