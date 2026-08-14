//! Project-level APP packaging compatible with hvigor 6.24.3.
//!
//! The project task graph keeps orchestration in the CLI, while this module
//! owns the typed inputs, pack-info merge, module staging, and SDK packing
//! command. The external SDK JAR is intentionally retained for APP archives:
//! it replaces each embedded HAP/HSP `pack.info` with the project document,
//! an archive rewrite whose byte-level behavior is part of the public format.
//!
//! Upstream owners:
//! - `tasks/make-project-pack-info.js`
//! - `tasks/generate-pack-res.js`
//! - `tasks/package-app.js`
//! - `builder/inner-java-command-builder/packing-tool-options.js`

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_path::HostPlatform;
use rayon::prelude::*;

use crate::error::PackError;
use crate::integrated_hsp::{normalize_integrated_hsps, IntegratedHspEntry};

/// Whether a project APP input was produced by a local module or discovered
/// from an installed remote HSP package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppArtifactRole {
    Local { module_name: String },
    RemoteHsp,
}

/// Logical package kind supplied to app-packing-tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppModulePackageKind {
    Hap,
    Hsp,
}

/// Physical representation accepted by hvigor's `PackageApp` task.
///
/// Normal builds stage HAP/HSP archives. `fastBuildApp` stages the directory
/// emitted by `FastPackageHap`/`FastPackageHsp` and invokes the SDK tool with
/// `--mode fastApp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppArtifactFormat {
    Archive,
    FastDirectory {
        kind: AppModulePackageKind,
        destination: PathBuf,
    },
}

/// One HAP/HSP (or fast-package directory) participating in project
/// `pack.info` and APP construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppArtifactInput {
    pub path: PathBuf,
    pub role: AppArtifactRole,
    pub format: AppArtifactFormat,
}

impl AppArtifactInput {
    pub fn local(path: PathBuf, module_name: impl Into<String>) -> Self {
        Self {
            path,
            role: AppArtifactRole::Local {
                module_name: module_name.into(),
            },
            format: AppArtifactFormat::Archive,
        }
    }

    pub fn local_fast_directory(
        path: PathBuf,
        module_name: impl Into<String>,
        kind: AppModulePackageKind,
        destination: PathBuf,
    ) -> Self {
        Self {
            path,
            role: AppArtifactRole::Local {
                module_name: module_name.into(),
            },
            format: AppArtifactFormat::FastDirectory { kind, destination },
        }
    }

    pub fn remote_hsp(path: PathBuf) -> Self {
        Self {
            path,
            role: AppArtifactRole::RemoteHsp,
            format: AppArtifactFormat::Archive,
        }
    }

    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(OsStr::to_str)
    }

    pub fn package_kind(&self) -> Result<AppModulePackageKind, PackError> {
        match &self.format {
            AppArtifactFormat::FastDirectory { kind, .. } => Ok(*kind),
            AppArtifactFormat::Archive => match self.extension() {
                Some("hap") => Ok(AppModulePackageKind::Hap),
                Some("hsp") => Ok(AppModulePackageKind::Hsp),
                extension => Err(PackError::InvalidAppPlan(format!(
                    "APP input {} must use .hap or .hsp, got {:?}",
                    self.path.display(),
                    extension
                ))),
            },
        }
    }
}

/// One module's dependency names and its module-local remote-HSP directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteHspDependencyScope {
    pub module_root: PathBuf,
    pub package_names: BTreeSet<String>,
}

impl RemoteHspDependencyScope {
    pub fn new(module_root: PathBuf, package_names: impl IntoIterator<Item = String>) -> Self {
        Self {
            module_root,
            package_names: package_names.into_iter().collect(),
        }
    }
}

/// Latest-hvigor remote HSP discovery for project APP assembly.
///
/// For each module, hvigor inspects `<module>/oh_modules/.hsp` followed by
/// `<project>/oh_modules/.hsp`, retains only package names present in that
/// module's dependency graph, and keeps the highest semantic version for a
/// repeated HSP filename.
pub struct RemoteHspCollector<'a> {
    project_root: &'a Path,
    scopes: &'a [RemoteHspDependencyScope],
}

impl<'a> RemoteHspCollector<'a> {
    pub fn new(project_root: &'a Path, scopes: &'a [RemoteHspDependencyScope]) -> Self {
        Self {
            project_root,
            scopes,
        }
    }

    pub fn collect_candidates(&self) -> Result<BTreeMap<String, RemoteHspCandidate>, PackError> {
        self.collect_candidates_impl()
    }

    /// Candidates owned by `SignProjectRemoteHsp`: only the project-level
    /// `oh_modules/.hsp` repository is inspected, once per module dependency
    /// scope.
    pub fn collect_project_candidates(
        &self,
    ) -> Result<BTreeMap<String, RemoteHspCandidate>, PackError> {
        let mut candidates = BTreeMap::new();
        for scope in self.scopes {
            self.scan_repository(
                &self.project_root.join("oh_modules").join(".hsp"),
                &scope.package_names,
                &mut candidates,
            )?;
        }
        Ok(candidates)
    }

    /// Candidates owned by `SignModuleRemoteHsp`: only each module-local
    /// `oh_modules/.hsp` repository is inspected. Project candidates are
    /// handled by the task dependency above.
    pub fn collect_module_candidates(
        &self,
    ) -> Result<BTreeMap<String, RemoteHspCandidate>, PackError> {
        let mut candidates = BTreeMap::new();
        for scope in self.scopes {
            self.scan_repository(
                &scope.module_root.join("oh_modules").join(".hsp"),
                &scope.package_names,
                &mut candidates,
            )?;
        }
        Ok(candidates)
    }

    pub fn collect_for_app(
        &self,
        bundle_name: &str,
        version_code: u64,
        build_version: Option<&str>,
        integrated_output_dir: &Path,
    ) -> Result<Vec<AppArtifactInput>, PackError> {
        let candidates = self.collect_candidates()?;
        let integrated_entries = candidates
            .values()
            .filter(|candidate| candidate.integrated)
            .map(|candidate| IntegratedHspEntry {
                hsp_dir_name: candidate.directory_name.clone(),
                hsp_file_name: candidate.file_name.clone(),
                hsp_path: candidate.path.clone(),
            })
            .collect::<Vec<_>>();
        let normalized = normalize_integrated_hsps(
            &integrated_entries,
            bundle_name,
            version_code,
            build_version,
            integrated_output_dir,
        )?;
        let normalized_by_name = integrated_entries
            .iter()
            .zip(normalized)
            .map(|(entry, path)| (entry.hsp_file_name.clone(), path))
            .collect::<BTreeMap<_, _>>();

        Ok(candidates
            .into_values()
            .map(|candidate| {
                let path = normalized_by_name
                    .get(&candidate.file_name)
                    .cloned()
                    .unwrap_or(candidate.path);
                AppArtifactInput::remote_hsp(path)
            })
            .collect())
    }

    fn collect_candidates_impl(&self) -> Result<BTreeMap<String, RemoteHspCandidate>, PackError> {
        let mut candidates = self.collect_module_candidates()?;
        for (name, candidate) in self.collect_project_candidates()? {
            let replace = candidates
                .get(&name)
                .map(|current| candidate.version >= current.version)
                .unwrap_or(true);
            if replace {
                candidates.insert(name, candidate);
            }
        }
        Ok(candidates)
    }

    fn scan_repository(
        &self,
        repository: &Path,
        package_names: &BTreeSet<String>,
        candidates: &mut BTreeMap<String, RemoteHspCandidate>,
    ) -> Result<(), PackError> {
        if !repository.exists() {
            return Ok(());
        }
        let mut directories = fs_err::read_dir(repository)?.collect::<Result<Vec<_>, _>>()?;
        directories.sort_by_key(|entry| entry.file_name());
        for directory in directories {
            let path = directory.path();
            if !directory.file_type()?.is_dir() {
                return Err(PackError::InvalidRemoteHsp {
                    path: path.display().to_string(),
                    message: "entries under oh_modules/.hsp must be directories".to_string(),
                });
            }
            let candidate = RemoteHspCandidate::read(&path)?;
            if !package_names.contains(&candidate.package_name) {
                continue;
            }
            let replace = candidates
                .get(&candidate.file_name)
                .map(|current| candidate.version >= current.version)
                .unwrap_or(true);
            if replace {
                candidates.insert(candidate.file_name.clone(), candidate);
            }
        }
        Ok(())
    }
}

pub struct RemoteHspCandidate {
    pub package_name: String,
    pub version: semver::Version,
    pub path: PathBuf,
    pub directory_name: String,
    pub file_name: String,
    pub integrated: bool,
}

impl RemoteHspCandidate {
    fn read(directory: &Path) -> Result<Self, PackError> {
        let manifest_path = directory.join("oh-package.json5");
        if !manifest_path.is_file() {
            return Err(PackError::InvalidRemoteHsp {
                path: directory.display().to_string(),
                message: "oh-package.json5 is missing".to_string(),
            });
        }
        let manifest_bytes = fs_err::read(&manifest_path)?;
        let manifest: serde_json::Value =
            serde_json::from_slice(&manifest_bytes).map_err(|error| {
                PackError::InvalidRemoteHsp {
                    path: manifest_path.display().to_string(),
                    message: error.to_string(),
                }
            })?;
        let package_name = manifest
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| PackError::InvalidRemoteHsp {
                path: manifest_path.display().to_string(),
                message: "package name is missing".to_string(),
            })?
            .to_string();
        let version_text = manifest
            .get("version")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| PackError::InvalidRemoteHsp {
                path: manifest_path.display().to_string(),
                message: "package version is missing".to_string(),
            })?;
        let version =
            semver::Version::parse(version_text).map_err(|error| PackError::InvalidRemoteHsp {
                path: manifest_path.display().to_string(),
                message: format!("illegal semantic version {version_text:?}: {error}"),
            })?;
        let integrated = manifest
            .pointer("/metadata/integratedHsp")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let mut files = fs_err::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        files.sort_by_key(|entry| entry.file_name());
        let hsp_files = files
            .into_iter()
            .filter(|entry| entry.path().extension().and_then(OsStr::to_str) == Some("hsp"))
            .collect::<Vec<_>>();
        if hsp_files.len() != 1 {
            return Err(PackError::InvalidRemoteHsp {
                path: directory.display().to_string(),
                message: format!(
                    "expected exactly one .hsp archive, found {}",
                    hsp_files.len()
                ),
            });
        }
        let hsp = &hsp_files[0];
        let file_name = hsp
            .file_name()
            .to_str()
            .ok_or_else(|| PackError::InvalidRemoteHsp {
                path: hsp.path().display().to_string(),
                message: "HSP filename is not valid UTF-8".to_string(),
            })?
            .to_string();
        let directory_name = directory
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| PackError::InvalidRemoteHsp {
                path: directory.display().to_string(),
                message: "HSP directory name is not valid UTF-8".to_string(),
            })?
            .to_string();
        Ok(Self {
            package_name,
            version,
            path: hsp.path(),
            directory_name,
            file_name,
            integrated,
        })
    }
}

/// Output from hvigor's `MakeProjectPackInfo` equivalent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPackInfoOutput {
    pub path: PathBuf,
    pub artifact_count: usize,
    /// Basenames selected by hvigor's aligned-device-type `getUnpackModules`.
    pub package_file_names: BTreeSet<String>,
}

/// Merges module package metadata into the project-level `pack.info`.
pub struct ProjectPackInfoBuilder<'a> {
    artifacts: &'a [AppArtifactInput],
    align_device_types: &'a [String],
}

impl<'a> ProjectPackInfoBuilder<'a> {
    pub fn new(artifacts: &'a [AppArtifactInput]) -> Self {
        Self {
            artifacts,
            align_device_types: &[],
        }
    }

    pub fn with_aligned_device_types(mut self, device_types: &'a [String]) -> Self {
        self.align_device_types = device_types;
        self
    }

    pub fn write(&self, output_path: &Path) -> Result<ProjectPackInfoOutput, PackError> {
        if self.artifacts.is_empty() {
            return Err(PackError::InvalidAppPlan(
                "MakeProjectPackInfo requires at least one HAP or HSP artifact".to_string(),
            ));
        }

        let mut accumulator = ProjectPackInfoAccumulator::new(self.align_device_types);
        for artifact in self.artifacts {
            self.validate_artifact(artifact)?;
            let pack_info = self.read_artifact_json(artifact, "pack.info")?;
            match &artifact.role {
                AppArtifactRole::Local { module_name } => {
                    accumulator.merge_local(&pack_info, module_name)?;
                }
                AppArtifactRole::RemoteHsp => {
                    let module_json = self.read_artifact_json(artifact, "module.json")?;
                    accumulator.merge_remote(&pack_info, &module_json, &artifact.path)?;
                }
            }
        }

        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let value = accumulator.finish();
        let package_file_names = Self::package_file_names(&value)?;
        let mut bytes = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
        let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
        serde::Serialize::serialize(&value, &mut serializer)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        // fs-extra outputJSONSync writes a trailing line feed.
        bytes.push(b'\n');
        fs_err::write(output_path, bytes)?;

        Ok(ProjectPackInfoOutput {
            path: output_path.to_path_buf(),
            artifact_count: self.artifacts.len(),
            package_file_names,
        })
    }

    pub fn read_package_file_names(pack_info_path: &Path) -> Result<BTreeSet<String>, PackError> {
        let bytes = fs_err::read(pack_info_path)?;
        let value = serde_json::from_slice(&bytes)
            .map_err(|error| PackError::MalformedPackInfo(error.to_string()))?;
        Self::package_file_names(&value)
    }

    fn package_file_names(pack_info: &serde_json::Value) -> Result<BTreeSet<String>, PackError> {
        let packages = pack_info
            .get("packages")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| PackError::MalformedPackInfo("packages must be an array".to_string()))?;
        packages
            .iter()
            .map(|package| {
                let name = package
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        PackError::MalformedPackInfo("packages[].name must be a string".to_string())
                    })?;
                let extension = if package
                    .get("moduleType")
                    .and_then(serde_json::Value::as_str)
                    == Some("shared")
                {
                    "hsp"
                } else {
                    "hap"
                };
                Ok(format!("{name}.{extension}"))
            })
            .collect()
    }

    fn validate_artifact(&self, artifact: &AppArtifactInput) -> Result<(), PackError> {
        match &artifact.format {
            AppArtifactFormat::Archive => {
                if !artifact.path.is_file() {
                    return Err(PackError::MissingInput(artifact.path.display().to_string()));
                }
                artifact.package_kind().map(|_| ())
            }
            AppArtifactFormat::FastDirectory { destination, .. } => {
                if !artifact.path.is_dir() {
                    return Err(PackError::MissingInput(artifact.path.display().to_string()));
                }
                if matches!(artifact.role, AppArtifactRole::RemoteHsp) {
                    return Err(PackError::InvalidAppPlan(
                        "remote HSP inputs cannot use the fast-package directory format"
                            .to_string(),
                    ));
                }
                if destination.file_name().is_none() {
                    return Err(PackError::InvalidAppPlan(format!(
                        "fast APP destination has no module name: {}",
                        destination.display()
                    )));
                }
                Ok(())
            }
        }
    }

    fn read_artifact_json(
        &self,
        artifact: &AppArtifactInput,
        entry_name: &str,
    ) -> Result<serde_json::Value, PackError> {
        match artifact.format {
            AppArtifactFormat::Archive => self.read_archive_json(&artifact.path, entry_name),
            AppArtifactFormat::FastDirectory { .. } => {
                let path = artifact.path.join(entry_name);
                let bytes = fs_err::read(&path).map_err(|error| {
                    PackError::InvalidAppPlan(format!(
                        "{} is missing {entry_name}: {error}",
                        artifact.path.display()
                    ))
                })?;
                serde_json::from_slice(&bytes).map_err(|error| {
                    PackError::MalformedPackInfo(format!("{}: {error}", path.display()))
                })
            }
        }
    }

    fn read_archive_json(
        &self,
        archive_path: &Path,
        entry_name: &str,
    ) -> Result<serde_json::Value, PackError> {
        let file = fs_err::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut entry = archive.by_name(entry_name).map_err(|error| {
            PackError::InvalidAppPlan(format!(
                "{} is missing {entry_name}: {error}",
                archive_path.display()
            ))
        })?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes).map_err(|error| {
            PackError::MalformedPackInfo(format!(
                "{}!/{entry_name}: {error}",
                archive_path.display()
            ))
        })
    }
}

struct ProjectPackInfoAccumulator<'a> {
    app: serde_json::Value,
    modules: Vec<serde_json::Value>,
    packages: Vec<serde_json::Value>,
    module_names: BTreeSet<String>,
    align_device_types: &'a [String],
}

impl<'a> ProjectPackInfoAccumulator<'a> {
    fn new(align_device_types: &'a [String]) -> Self {
        Self {
            app: serde_json::json!({
                "bundleName": "bundleName",
                "version": { "name": "name", "code": 0 }
            }),
            modules: Vec::new(),
            packages: Vec::new(),
            module_names: BTreeSet::new(),
            align_device_types,
        }
    }

    fn merge_local(
        &mut self,
        pack_info: &serde_json::Value,
        module_name: &str,
    ) -> Result<(), PackError> {
        let module = self.first_module(pack_info)?.clone();
        if !self.includes_aligned_device_types(&module) {
            return Ok(());
        }
        if pack_info
            .pointer("/summary/app/bundleName")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| !name.is_empty())
        {
            self.app = pack_info.pointer("/summary/app").cloned().ok_or_else(|| {
                PackError::MalformedPackInfo("summary.app must be an object".to_string())
            })?;
        }
        self.push_module_once(module_name, module);
        self.append_packages(pack_info, None)
    }

    fn merge_remote(
        &mut self,
        pack_info: &serde_json::Value,
        module_json: &serde_json::Value,
        archive_path: &Path,
    ) -> Result<(), PackError> {
        let module_name = module_json
            .pointer("/module/name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                PackError::MalformedPackInfo(format!(
                    "{}!/module.json is missing module.name",
                    archive_path.display()
                ))
            })?;
        let module = self.first_module(pack_info)?.clone();
        if !self.includes_aligned_device_types(&module) {
            return Ok(());
        }
        self.push_module_once(module_name, module);
        let archive_name = archive_path
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| {
                PackError::InvalidAppPlan(format!(
                    "remote HSP filename is not valid UTF-8: {}",
                    archive_path.display()
                ))
            })?;
        self.append_packages(pack_info, Some(archive_name))
    }

    fn first_module<'b>(
        &self,
        pack_info: &'b serde_json::Value,
    ) -> Result<&'b serde_json::Value, PackError> {
        pack_info.pointer("/summary/modules/0").ok_or_else(|| {
            PackError::MalformedPackInfo("summary.modules[0] is missing".to_string())
        })
    }

    fn includes_aligned_device_types(&self, module: &serde_json::Value) -> bool {
        if self.align_device_types.is_empty() {
            return true;
        }
        let Some(device_types) = module
            .get("deviceType")
            .and_then(serde_json::Value::as_array)
        else {
            return false;
        };
        self.align_device_types.iter().all(|required| {
            device_types
                .iter()
                .any(|actual| actual.as_str() == Some(required.as_str()))
        })
    }

    fn push_module_once(&mut self, module_name: &str, mut module: serde_json::Value) {
        if !self.module_names.insert(module_name.to_string()) {
            return;
        }
        self.normalize_device_types(&mut module);
        self.modules.push(module);
    }

    fn append_packages(
        &mut self,
        pack_info: &serde_json::Value,
        remote_archive_name: Option<&str>,
    ) -> Result<(), PackError> {
        let packages = pack_info
            .get("packages")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| PackError::MalformedPackInfo("packages must be an array".to_string()))?;
        for (index, package) in packages.iter().enumerate() {
            let mut package = package.clone();
            if index == 0 {
                if let Some(remote_archive_name) = remote_archive_name {
                    package["name"] = remote_archive_name.into();
                }
            }
            self.normalize_device_types(&mut package);
            self.packages.push(package);
        }
        Ok(())
    }

    fn normalize_device_types(&self, value: &mut serde_json::Value) {
        if !self.align_device_types.is_empty() {
            value["deviceType"] = serde_json::json!(self.align_device_types);
        }
    }

    fn finish(self) -> serde_json::Value {
        serde_json::json!({
            "summary": {
                "app": self.app,
                "modules": self.modules,
            },
            "packages": self.packages,
        })
    }
}

/// Result of copying local module outputs to each hvigor `outputs/<target>/app`
/// directory. Remote HSP inputs remain at their package/cache path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedAppArtifacts {
    pub artifacts: Vec<AppArtifactInput>,
    pub hap_paths: Vec<PathBuf>,
    pub hsp_paths: Vec<PathBuf>,
}

/// Result of unpacking an APP before signing its embedded HAP/HSP packages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackedAppArtifacts {
    pub root_path: PathBuf,
    pub hap_paths: Vec<PathBuf>,
    pub hsp_paths: Vec<PathBuf>,
}

/// Safe Rust equivalent of hvigor's `app_unpacking_tool.jar --mode app` use in
/// `SignPackagesFromApp`.
pub struct AppArchiveUnpacker<'a> {
    app_path: &'a Path,
}

impl<'a> AppArchiveUnpacker<'a> {
    pub fn new(app_path: &'a Path) -> Self {
        Self { app_path }
    }

    pub fn unpack(&self) -> Result<UnpackedAppArtifacts, PackError> {
        if !self.app_path.is_file() {
            return Err(PackError::MissingInput(self.app_path.display().to_string()));
        }
        if self.app_path.extension().and_then(OsStr::to_str) != Some("app") {
            return Err(PackError::InvalidAppPlan(format!(
                "SignPackagesFromApp input must use .app: {}",
                self.app_path.display()
            )));
        }

        let root_path = self.app_path.with_extension("");
        if root_path.exists() {
            fs_err::remove_dir_all(&root_path)?;
        }
        fs_err::create_dir_all(&root_path)?;

        let result = self.extract_into(&root_path);
        if result.is_err() {
            let _ = fs_err::remove_dir_all(&root_path);
        }
        result
    }

    pub fn cleanup(&self, unpacked: &UnpackedAppArtifacts) -> Result<(), PackError> {
        let expected_root = self.app_path.with_extension("");
        if unpacked.root_path != expected_root {
            return Err(PackError::InvalidAppPlan(format!(
                "refusing to remove unexpected APP unpack directory: {}",
                unpacked.root_path.display()
            )));
        }
        if unpacked.root_path.exists() {
            fs_err::remove_dir_all(&unpacked.root_path)?;
        }
        Ok(())
    }

    fn extract_into(&self, root_path: &Path) -> Result<UnpackedAppArtifacts, PackError> {
        let file = fs_err::File::open(self.app_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut hap_paths = Vec::new();
        let mut hsp_paths = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let relative = entry.enclosed_name().ok_or_else(|| {
                PackError::InvalidAppPlan(format!(
                    "APP contains an unsafe archive path: {}",
                    entry.name()
                ))
            })?;
            let output = root_path.join(relative);
            if entry.is_dir() {
                fs_err::create_dir_all(&output)?;
                continue;
            }
            if let Some(parent) = output.parent() {
                fs_err::create_dir_all(parent)?;
            }
            let mut writer = fs_err::File::create(&output)?;
            std::io::copy(&mut entry, &mut writer)?;
            writer.flush()?;
            match output.extension().and_then(OsStr::to_str) {
                Some("hap") => hap_paths.push(output),
                Some("hsp") => hsp_paths.push(output),
                _ => {}
            }
        }
        if hap_paths.is_empty() && hsp_paths.is_empty() {
            return Err(PackError::InvalidAppPlan(format!(
                "APP contains no HAP or HSP packages: {}",
                self.app_path.display()
            )));
        }
        Ok(UnpackedAppArtifacts {
            root_path: root_path.to_path_buf(),
            hap_paths,
            hsp_paths,
        })
    }
}

pub struct AppArtifactStager<'a> {
    artifacts: &'a [AppArtifactInput],
}

impl<'a> AppArtifactStager<'a> {
    pub fn new(artifacts: &'a [AppArtifactInput]) -> Self {
        Self { artifacts }
    }

    pub fn stage(&self) -> Result<StagedAppArtifacts, PackError> {
        let mut hap_names = BTreeSet::new();
        let mut hsp_names = BTreeSet::new();
        let mut plans = Vec::with_capacity(self.artifacts.len());
        for artifact in self.artifacts {
            let kind = artifact.package_kind()?;
            let destination = self.destination(artifact)?;
            let output_name = destination
                .file_name()
                .and_then(OsStr::to_str)
                .ok_or_else(|| {
                    PackError::InvalidAppPlan(format!(
                        "APP artifact output name is not valid UTF-8: {}",
                        destination.display()
                    ))
                })?;
            let names = if kind == AppModulePackageKind::Hap {
                &mut hap_names
            } else {
                &mut hsp_names
            };
            if !names.insert(output_name.to_string()) {
                return Err(PackError::DuplicateAppArtifactName(output_name.to_string()));
            }
            plans.push((artifact.clone(), destination));
        }

        let staged = plans
            .par_iter()
            .map(|(artifact, destination)| {
                if matches!(artifact.role, AppArtifactRole::Local { .. }) {
                    let parent = destination.parent().ok_or_else(|| {
                        PackError::InvalidAppPlan(format!(
                            "staged APP artifact has no parent: {}",
                            destination.display()
                        ))
                    })?;
                    fs_err::create_dir_all(parent)?;
                    match artifact.format {
                        AppArtifactFormat::Archive => {
                            fs_err::copy(&artifact.path, destination)?;
                        }
                        AppArtifactFormat::FastDirectory { .. } => {
                            Self::copy_directory(&artifact.path, destination)?;
                        }
                    }
                }
                Ok(AppArtifactInput {
                    path: destination.clone(),
                    role: artifact.role.clone(),
                    format: artifact.format.clone(),
                })
            })
            .collect::<Result<Vec<_>, PackError>>()?;

        let mut hap_paths = Vec::new();
        let mut hsp_paths = Vec::new();
        for artifact in &staged {
            match artifact.package_kind()? {
                AppModulePackageKind::Hap => hap_paths.push(artifact.path.clone()),
                AppModulePackageKind::Hsp => hsp_paths.push(artifact.path.clone()),
            }
        }
        Ok(StagedAppArtifacts {
            artifacts: staged,
            hap_paths,
            hsp_paths,
        })
    }

    fn destination(&self, artifact: &AppArtifactInput) -> Result<PathBuf, PackError> {
        match (&artifact.role, &artifact.format) {
            (AppArtifactRole::RemoteHsp, AppArtifactFormat::Archive) => Ok(artifact.path.clone()),
            (AppArtifactRole::RemoteHsp, AppArtifactFormat::FastDirectory { .. }) => {
                Err(PackError::InvalidAppPlan(
                    "remote HSP inputs cannot use the fast-package directory format".to_string(),
                ))
            }
            (
                AppArtifactRole::Local { .. },
                AppArtifactFormat::FastDirectory { destination, .. },
            ) => Ok(destination.clone()),
            (AppArtifactRole::Local { .. }, AppArtifactFormat::Archive) => {
                let file_name = artifact
                    .path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .ok_or_else(|| {
                        PackError::InvalidAppPlan(format!(
                            "APP artifact filename is not valid UTF-8: {}",
                            artifact.path.display()
                        ))
                    })?;
                let parent = artifact.path.parent().ok_or_else(|| {
                    PackError::InvalidAppPlan(format!(
                        "APP artifact has no output directory: {}",
                        artifact.path.display()
                    ))
                })?;
                Ok(parent
                    .join("app")
                    .join(file_name.replacen("-unsigned", "", 1)))
            }
        }
    }

    fn copy_directory(source: &Path, destination: &Path) -> Result<(), PackError> {
        fs_err::create_dir_all(destination)?;
        for entry in walkdir::WalkDir::new(source) {
            let entry = entry?;
            let relative = entry.path().strip_prefix(source).map_err(|error| {
                PackError::InvalidAppPlan(format!(
                    "cannot stage fast APP directory {}: {error}",
                    source.display()
                ))
            })?;
            let output = destination.join(relative);
            if entry.file_type().is_dir() {
                fs_err::create_dir_all(&output)?;
            } else if entry.file_type().is_file() {
                if let Some(parent) = output.parent() {
                    fs_err::create_dir_all(parent)?;
                }
                fs_err::copy(entry.path(), output)?;
            }
        }
        Ok(())
    }
}

/// SDK APP packaging mode selected by hvigor `PackageApp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPackageMode {
    App,
    FastApp,
}

impl AppPackageMode {
    fn as_sdk_argument(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::FastApp => "fastApp",
        }
    }
}

/// Typed input to the SDK `--mode app` / `--mode fastApp` operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPackagePlan {
    pub mode: AppPackageMode,
    pub pack_info_path: PathBuf,
    pub hap_paths: Vec<PathBuf>,
    pub hsp_paths: Vec<PathBuf>,
    pub output_path: PathBuf,
    pub pac_json_path: Option<PathBuf>,
    pub pack_res_path: Option<PathBuf>,
    pub compress_level: Option<String>,
    pub replace_pack_info: Option<bool>,
    pub main_module_limit: usize,
    pub normal_module_limit: usize,
}

/// Typed SDK `app_packing_tool.jar --mode hqf` input used by
/// `PackageHqf.getColdReloadCommand()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HqfPackagePlan {
    pub patch_json_path: PathBuf,
    pub native_libs_path: PathBuf,
    pub ets_path: PathBuf,
    pub resources_path: PathBuf,
    pub output_path: PathBuf,
}

impl HqfPackagePlan {
    pub fn validate(&self) -> Result<(), PackError> {
        if !self.patch_json_path.is_file() {
            return Err(PackError::MissingInput(
                self.patch_json_path.display().to_string(),
            ));
        }
        for directory in [&self.native_libs_path, &self.ets_path, &self.resources_path] {
            if !directory.is_dir() {
                return Err(PackError::MissingInput(directory.display().to_string()));
            }
        }
        Ok(())
    }
}

impl AppPackagePlan {
    pub fn new(
        pack_info_path: PathBuf,
        hap_paths: Vec<PathBuf>,
        hsp_paths: Vec<PathBuf>,
        output_path: PathBuf,
    ) -> Self {
        Self {
            mode: AppPackageMode::App,
            pack_info_path,
            hap_paths,
            hsp_paths,
            output_path,
            pac_json_path: None,
            pack_res_path: None,
            compress_level: None,
            replace_pack_info: None,
            main_module_limit: 2,
            normal_module_limit: 2,
        }
    }

    pub fn validate(&self) -> Result<(), PackError> {
        if !self.pack_info_path.is_file() {
            return Err(PackError::MissingInput(
                self.pack_info_path.display().to_string(),
            ));
        }
        if self.hap_paths.is_empty() && self.hsp_paths.is_empty() {
            return Err(PackError::InvalidAppPlan(
                "PackageApp requires at least one HAP or HSP".to_string(),
            ));
        }
        for path in self.hap_paths.iter().chain(&self.hsp_paths) {
            let valid_input = match self.mode {
                AppPackageMode::App => path.is_file(),
                AppPackageMode::FastApp => path.is_file() || path.is_dir(),
            };
            if !valid_input {
                return Err(PackError::MissingInput(path.display().to_string()));
            }
            self.validate_comma_free_path(path)?;
        }
        self.validate_comma_free_path(&self.pack_info_path)?;
        self.validate_comma_free_path(&self.output_path)?;
        if let Some(path) = &self.pac_json_path {
            if !path.is_file() {
                return Err(PackError::MissingInput(path.display().to_string()));
            }
            self.validate_comma_free_path(path)?;
        }
        if let Some(path) = &self.pack_res_path {
            if !path.is_file() {
                return Err(PackError::MissingInput(path.display().to_string()));
            }
            self.validate_comma_free_path(path)?;
        }
        Ok(())
    }

    fn validate_comma_free_path(&self, path: &Path) -> Result<(), PackError> {
        let text = path.to_str().ok_or_else(|| {
            PackError::InvalidAppPlan(format!(
                "SDK APP packing requires a UTF-8 path: {}",
                path.display()
            ))
        })?;
        if text.contains(',') {
            return Err(PackError::InvalidAppPlan(format!(
                "SDK APP packing cannot represent a comma in path: {}",
                path.display()
            )));
        }
        Ok(())
    }
}

/// Typed input to app-packing-tool's `--mode generalNormalize` operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppNormalizePlan {
    pub input_paths: Vec<PathBuf>,
    pub device_types: Vec<String>,
    pub output_dir: PathBuf,
}

impl AppNormalizePlan {
    pub fn new(input_paths: Vec<PathBuf>, device_types: Vec<String>, output_dir: PathBuf) -> Self {
        Self {
            input_paths,
            device_types,
            output_dir,
        }
    }

    fn validate(&self) -> Result<(), PackError> {
        if self.input_paths.is_empty() {
            return Err(PackError::InvalidAppPlan(
                "generalNormalize requires at least one HAP or HSP".to_string(),
            ));
        }
        if self.device_types.is_empty() {
            return Err(PackError::InvalidAppPlan(
                "generalNormalize requires at least one device type".to_string(),
            ));
        }
        for path in &self.input_paths {
            if !path.is_file() {
                return Err(PackError::MissingInput(path.display().to_string()));
            }
            self.validate_comma_free_path(path)?;
        }
        self.validate_comma_free_path(&self.output_dir)
    }

    fn validate_comma_free_path(&self, path: &Path) -> Result<(), PackError> {
        let text = path.to_str().ok_or_else(|| {
            PackError::InvalidAppPlan(format!(
                "SDK APP normalization requires a UTF-8 path: {}",
                path.display()
            ))
        })?;
        if text.contains(',') {
            return Err(PackError::InvalidAppPlan(format!(
                "SDK APP normalization cannot represent a comma in path: {}",
                path.display()
            )));
        }
        Ok(())
    }
}

/// Cross-platform process specification exposed for parity tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPackingCommand {
    program: OsString,
    args: Vec<OsString>,
}

impl AppPackingCommand {
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    fn execute(&self, output_path: &Path) -> Result<(), PackError> {
        let output = Command::new(&self.program).args(&self.args).output()?;
        if output.status.success() {
            return Ok(());
        }
        let _ = std::fs::remove_file(output_path);
        Err(PackError::AppPackingToolFailed {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn execute_hqf(&self, output_path: &Path) -> Result<(), PackError> {
        let output = Command::new(&self.program).args(&self.args).output()?;
        if output.status.success() {
            return Ok(());
        }
        let _ = std::fs::remove_file(output_path);
        Err(PackError::HqfPackingToolFailed {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Executes hvigor-compatible APP and EntryCard resource packing commands.
#[derive(Debug, Clone)]
pub struct AppPackageRunner {
    package_tool_path: PathBuf,
    java_executable: OsString,
    host: HostPlatform,
}

impl AppPackageRunner {
    pub fn new(package_tool_path: PathBuf) -> Self {
        let host = HostPlatform::CURRENT;
        let executable = OsString::from(host.executable_name("java"));
        let java_executable = std::env::var_os("JAVA_HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from)
            .map(|home| home.join("bin").join(&executable))
            .filter(|path| path.is_file())
            .map(PathBuf::into_os_string)
            .unwrap_or(executable);
        Self::for_host(package_tool_path, java_executable, host)
    }

    pub fn for_host(
        package_tool_path: PathBuf,
        java_executable: OsString,
        host: HostPlatform,
    ) -> Self {
        Self {
            package_tool_path,
            java_executable,
            host,
        }
    }

    pub fn pack(&self, plan: &AppPackagePlan) -> Result<(), PackError> {
        let command = self.pack_command(plan)?;
        if let Some(parent) = plan.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        command.execute(&plan.output_path)
    }

    pub fn pack_hqf(&self, plan: &HqfPackagePlan) -> Result<(), PackError> {
        let command = self.hqf_command(plan)?;
        if let Some(parent) = plan.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        command.execute_hqf(&plan.output_path)
    }

    pub fn generate_pack_res(
        &self,
        entry_card_dir: &Path,
        pack_info_path: &Path,
        output_path: &Path,
    ) -> Result<(), PackError> {
        let command = self.pack_res_command(entry_card_dir, pack_info_path, output_path)?;
        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        command.execute(output_path)
    }

    pub fn normalize_device_types(&self, plan: &AppNormalizePlan) -> Result<(), PackError> {
        let command = self.normalize_device_types_command(plan)?;
        if plan.output_dir.exists() {
            fs_err::remove_dir_all(&plan.output_dir)?;
        }
        fs_err::create_dir_all(&plan.output_dir)?;
        command.execute(&plan.output_dir)
    }

    pub fn pack_command(&self, plan: &AppPackagePlan) -> Result<AppPackingCommand, PackError> {
        plan.validate()?;
        self.validate_package_tool()?;
        let mut command = self.base_command();
        if let Some(pack_res_path) = &plan.pack_res_path {
            command.args.push("--pack-res-path".into());
            command.args.push(pack_res_path.as_os_str().to_owned());
        }
        command
            .args
            .extend(["--mode".into(), plan.mode.as_sdk_argument().into()]);
        command.args.push("--pack-info-path".into());
        command
            .args
            .push(plan.pack_info_path.as_os_str().to_owned());
        command.args.push("--hap-path".into());
        command.args.push(self.join_paths(&plan.hap_paths)?);
        command.args.push("--hsp-path".into());
        command.args.push(self.join_paths(&plan.hsp_paths)?);
        command.args.extend(["--force".into(), "true".into()]);
        if let Some(replace_pack_info) = plan.replace_pack_info {
            command.args.push("--replace-pack-info".into());
            command.args.push(replace_pack_info.to_string().into());
        }
        command.args.push("--out-path".into());
        command.args.push(plan.output_path.as_os_str().to_owned());
        command.args.push("--main-module-limit".into());
        command.args.push(plan.main_module_limit.to_string().into());
        command.args.push("--normal-module-limit".into());
        command
            .args
            .push(plan.normal_module_limit.to_string().into());
        if let Some(compress_level) = &plan.compress_level {
            command.args.push("--compress-level".into());
            command.args.push(compress_level.into());
        }
        if let Some(pac_json_path) = &plan.pac_json_path {
            command.args.push("--pac-json-path".into());
            command.args.push(pac_json_path.as_os_str().to_owned());
        }
        Ok(command)
    }

    pub fn hqf_command(&self, plan: &HqfPackagePlan) -> Result<AppPackingCommand, PackError> {
        plan.validate()?;
        self.validate_package_tool()?;
        Ok(AppPackingCommand {
            program: self.java_executable.clone(),
            args: vec![
                "-jar".into(),
                self.package_tool_path.as_os_str().to_owned(),
                "--mode".into(),
                "hqf".into(),
                "--json-path".into(),
                plan.patch_json_path.as_os_str().to_owned(),
                "--lib-path".into(),
                plan.native_libs_path.as_os_str().to_owned(),
                "--ets-path".into(),
                plan.ets_path.as_os_str().to_owned(),
                "--resources-path".into(),
                plan.resources_path.as_os_str().to_owned(),
                "--out-path".into(),
                plan.output_path.as_os_str().to_owned(),
                "--force".into(),
                "true".into(),
            ],
        })
    }

    pub fn pack_res_command(
        &self,
        entry_card_dir: &Path,
        pack_info_path: &Path,
        output_path: &Path,
    ) -> Result<AppPackingCommand, PackError> {
        self.validate_package_tool()?;
        for path in [entry_card_dir, pack_info_path] {
            if !path.exists() {
                return Err(PackError::MissingInput(path.display().to_string()));
            }
        }
        let mut command = self.base_command();
        command.args.extend(["--mode".into(), "res".into()]);
        command.args.push("--entrycard-path".into());
        command.args.push(entry_card_dir.as_os_str().to_owned());
        command.args.push("--pack-info-path".into());
        command.args.push(pack_info_path.as_os_str().to_owned());
        command.args.push("--out-path".into());
        command.args.push(output_path.as_os_str().to_owned());
        command.args.extend(["--force".into(), "true".into()]);
        Ok(command)
    }

    pub fn normalize_device_types_command(
        &self,
        plan: &AppNormalizePlan,
    ) -> Result<AppPackingCommand, PackError> {
        plan.validate()?;
        self.validate_package_tool()?;
        let mut command = self.base_command();
        command
            .args
            .extend(["--mode".into(), "generalNormalize".into()]);
        command.args.push("--device-types".into());
        command.args.push(plan.device_types.join(",").into());
        command.args.push("--input-list".into());
        command.args.push(self.join_paths(&plan.input_paths)?);
        command.args.push("--out-path".into());
        command.args.push(plan.output_dir.as_os_str().to_owned());
        Ok(command)
    }

    fn validate_package_tool(&self) -> Result<(), PackError> {
        if self.package_tool_path.is_file() {
            Ok(())
        } else {
            Err(PackError::MissingInput(
                self.package_tool_path.display().to_string(),
            ))
        }
    }

    fn base_command(&self) -> AppPackingCommand {
        let encoding = if self.host == HostPlatform::Windows {
            "-Dfile.encoding=GBK"
        } else {
            "-Dfile.encoding=utf-8"
        };
        AppPackingCommand {
            program: self.java_executable.clone(),
            args: vec![
                encoding.into(),
                "-jar".into(),
                self.package_tool_path.as_os_str().to_owned(),
            ],
        }
    }

    fn join_paths(&self, paths: &[PathBuf]) -> Result<OsString, PackError> {
        let mut joined = String::new();
        for (index, path) in paths.iter().enumerate() {
            let text = path.to_str().ok_or_else(|| {
                PackError::InvalidAppPlan(format!(
                    "SDK APP packing requires a UTF-8 path: {}",
                    path.display()
                ))
            })?;
            if index > 0 {
                joined.push(',');
            }
            joined.push_str(text);
        }
        Ok(joined.into())
    }
}
