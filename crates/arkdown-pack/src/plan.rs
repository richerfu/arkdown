//! Package plan types for HAP/HSP/HAR archive construction.
//!
//! A [`PackagePlan`] is the single authoritative input to the packaging stage.
//! It carries all pre-resolved paths and metadata that the packager needs to
//! assemble the final archive — no late reads of `module.json5`, `app.json5`,
//! or `oh-package.json5` are permitted inside `arkdown-pack`.
//!
//! # Upstream correspondence
//!
//! | hvigor task / file                                  | Rust type            |
//! |-----------------------------------------------------|----------------------|
//! | `PackageHap` (`package-hap.js`)                     | `PackagePlan` (HAP)  |
//! | `PackageHsp` (`package-hsp.js`)                     | `PackagePlan` (HSP)  |
//! | `PackageHar` (`package-har.js`)                     | `PackagePlan` (HAR)  |
//! | `GeneratePkgModuleJson → ProcessCompiledResources`  | `PackageEntry` edges |
//! | `ProcessIntegratedHsp`                              | `PackageEntry` (HSP) |
//! | `packing-tool-options.js`                           | `PackageToolMode`    |
//!
//! # Pipeline position
//!
//! ```text
//! profile/resource/bundle/abc outputs
//!   → PackagePlan (assembled by build-service BuildPlanBuilder)
//!   → arkdown-pack::Packager
//!   → unsigned HAP/HSP/HAR
//!   → arkdown-sign
//! ```

use std::path::PathBuf;

use crate::error::PackError;

// ---------------------------------------------------------------------------
// PackageKind
// ---------------------------------------------------------------------------

/// The output artifact kind for this packaging run.
///
/// Determines the archive layout and which entries are required.
///
/// hvigor upstream: `assembleHap` / `assembleHsp` / `assembleHar` task
/// discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackageKind {
    /// HAP — Harmony Application Package.
    ///
    /// Requires `module.json`, `resources.index`, `ets/modules.abc`, and
    /// `pack.info`. Native `.so` libraries and integrated HSPs are optional.
    Hap,
    /// HSP — Harmony Shared Package.
    ///
    /// Same archive layout as HAP; the `module.type` field in `module.json`
    /// distinguishes it at runtime.
    Hsp,
    /// HAR — Harmony Archive (source or bytecode library).
    ///
    /// Contains source `.ets` files, resources, `oh-package.json5`, and
    /// optional declaration stubs. Does **not** contain `pack.info`.
    Har,
}

impl PackageKind {
    /// Returns `true` if this kind produces a `pack.info` entry.
    ///
    /// Only HAP and HSP archives include `pack.info`; HAR does not.
    pub fn needs_pack_info(self) -> bool {
        matches!(self, PackageKind::Hap | PackageKind::Hsp)
    }

    /// Returns `true` if this kind requires a compiled `modules.abc` entry.
    pub fn needs_abc(self) -> bool {
        matches!(self, PackageKind::Hap | PackageKind::Hsp)
    }

    /// Returns the canonical file extension for the output artifact.
    pub fn extension(self) -> &'static str {
        match self {
            PackageKind::Hap => "hap",
            PackageKind::Hsp => "hsp",
            PackageKind::Har => "har",
        }
    }
}

// ---------------------------------------------------------------------------
// PackageToolMode
// ---------------------------------------------------------------------------

/// Controls whether the packager runs in-process or delegates to an external
/// packing tool subprocess.
///
/// hvigor upstream: `packing-tool-options.js` `PackingMode` enum.
///
/// In `arkdown`, the packager is always pure Rust (`InProcess`). The
/// `Subprocess` variant is retained for future compatibility with SDK
/// packing-tool binaries during a migration window.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PackageToolMode {
    /// Run the pure-Rust packager in-process (default).
    ///
    /// No external process is spawned; all archive construction happens inside
    /// the current process using the `zip` crate.
    #[default]
    InProcess,
    /// Delegate to the SDK `packing-tool` binary via subprocess.
    ///
    /// `tool_path` must point to the `packing-tool` executable from the OHOS
    /// SDK. Used only when `ARKDOWN_PACKING_TOOL` is set or the typed build
    /// request selects the external tool.
    Subprocess {
        /// Absolute path to the SDK `packing-tool` binary.
        tool_path: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// PackageEntrySource
// ---------------------------------------------------------------------------

/// The on-disk source of a single entry inside the output archive.
///
/// Entries whose source is `Generated` are assembled in-memory by the packager
/// (e.g. `pack.info`). All other entries originate from files on disk.
///
/// hvigor upstream: individual task outputs fed into `PackageHap`/`PackageHar`
/// via the task dependency graph.
#[derive(Debug, Clone)]
pub enum PackageEntrySource {
    /// A file that already exists on disk at the given path.
    ///
    /// The packager reads it verbatim and stores or deflates it according to
    /// the entry's compression policy.
    File(PathBuf),
    /// A directory whose contents are recursively walked and added.
    ///
    /// Archive paths for each file are constructed by stripping the directory
    /// prefix and prepending `archive_prefix`.
    Directory {
        /// Root directory to walk.
        root: PathBuf,
        /// Prefix prepended to each file's relative path inside the archive.
        ///
        /// Example: `"resources/"` for HAP resource files.
        archive_prefix: String,
    },
    /// Content generated in-memory by the packager itself.
    ///
    /// Used for `pack.info` and `pkgContextInfo.json` which are assembled from
    /// structured data rather than read from disk.
    Generated,
}

// ---------------------------------------------------------------------------
// PackageEntry
// ---------------------------------------------------------------------------

/// A single logical entry (or entry group) to be written into the output
/// archive.
///
/// Entries are collected by [`PackagePlan`] and consumed by the packager in
/// [`PackagePlan::ordered_entries`] order, which matches hvigor's deterministic
/// archive write sequence.
///
/// hvigor upstream: each `addToZip` / `addFileToHap` call in
/// `PackageHap`/`PackageHsp`/`PackageHar`.
#[derive(Debug, Clone)]
pub struct PackageEntry {
    /// Archive path for the entry (slash-separated, no leading `/`).
    ///
    /// For `Directory` sources this is the `archive_prefix`; for `File`
    /// sources this is the exact archive path.
    pub archive_path: String,
    /// Where the entry's bytes come from.
    pub source: PackageEntrySource,
    /// Whether this entry is required.
    ///
    /// If `true` and the source file is missing, [`PackagePlan::validate`]
    /// returns an error. Optional entries (e.g. `ets/sourceMaps.map`) are
    /// silently omitted when absent.
    pub required: bool,
}

impl PackageEntry {
    /// Create a required file entry.
    pub fn required_file(archive_path: impl Into<String>, path: PathBuf) -> Self {
        PackageEntry {
            archive_path: archive_path.into(),
            source: PackageEntrySource::File(path),
            required: true,
        }
    }

    /// Create an optional file entry (silently omitted when the file is
    /// absent).
    pub fn optional_file(archive_path: impl Into<String>, path: PathBuf) -> Self {
        PackageEntry {
            archive_path: archive_path.into(),
            source: PackageEntrySource::File(path),
            required: false,
        }
    }

    /// Create a directory entry whose contents are recursively walked.
    pub fn directory(archive_prefix: impl Into<String>, root: PathBuf) -> Self {
        let prefix = archive_prefix.into();
        PackageEntry {
            archive_path: prefix.clone(),
            source: PackageEntrySource::Directory {
                root,
                archive_prefix: prefix,
            },
            required: false,
        }
    }

    /// Create a generated (in-memory) entry.
    pub fn generated(archive_path: impl Into<String>) -> Self {
        PackageEntry {
            archive_path: archive_path.into(),
            source: PackageEntrySource::Generated,
            required: false,
        }
    }
}

// ---------------------------------------------------------------------------
// PackageOutput
// ---------------------------------------------------------------------------

/// Describes the output file produced by a packaging run.
///
/// hvigor upstream: the output path written by `PackageHap` / `PackageHar` and
/// consumed by `SignHap` / `AssembleHap`.
#[derive(Debug, Clone)]
pub struct PackageOutput {
    /// Absolute path where the unsigned archive will be written.
    ///
    /// For HAP/HSP this is the pre-signing artifact; for HAR it is the final
    /// artifact (HAR is not signed).
    pub path: PathBuf,
    /// The kind of artifact at `path`.
    pub kind: PackageKind,
    /// Whether the artifact at `path` has been signed.
    ///
    /// Always `false` immediately after packaging; set to `true` by
    /// `arkdown-sign` after the signing step completes.
    pub signed: bool,
}

impl PackageOutput {
    /// Create a new unsigned output descriptor.
    pub fn new(path: PathBuf, kind: PackageKind) -> Self {
        PackageOutput {
            path,
            kind,
            signed: false,
        }
    }
}

// ---------------------------------------------------------------------------
// PackagePlan
// ---------------------------------------------------------------------------

/// The complete, pre-resolved input to the packaging stage.
///
/// A `PackagePlan` is constructed by the build service's `BuildPlanBuilder` after all
/// upstream tasks (`ArkCompile`, `ProcessCompiledResources`,
/// `GeneratePkgModuleJson`, `ProcessIntegratedHsp`, …) have completed.  All
/// paths are absolute and already exist on disk (or will exist by the time
/// the packager runs); no manifest files are re-read inside `arkdown-pack`.
///
/// # hvigor correspondence
///
/// The plan represents the input edges of the following hvigor task chain:
///
/// ```text
/// GeneratePkgModuleJson
///   → ProcessCompiledResources
///   → PackageHap          (HAP/HSP)
///
/// ProcessHarArtifacts
///   → PackageHar          (HAR)
/// ```
///
/// For HSP builds, the plan also covers the side-channel outputs:
///
/// ```text
/// PrepareSharedHarRes → PackageSharedHar → PackageSharedTgz
/// ```
#[derive(Debug, Clone)]
pub struct PackagePlan {
    /// The name of the module being packaged (e.g. `"entry"`, `"shared"`).
    ///
    /// Used in `pack.info` `distro.moduleName` and as the archive stem.
    pub module_name: String,
    /// The output artifact kind.
    pub kind: PackageKind,
    /// Absolute path for the unsigned output archive.
    ///
    /// Parent directories are created by the packager if they do not exist.
    pub output_path: PathBuf,
    /// Ordered list of entries to include in the archive.
    ///
    /// The build-service populates this list in hvigor's archive-write order.  Callers
    /// should use [`PackagePlan::ordered_entries`] to iterate rather than
    /// accessing this field directly.
    pub entries: Vec<PackageEntry>,
    /// Compile API version from `SdkConfig.compile_api_version`.
    ///
    /// Determines the `ets/modules.abc` vs `modules.abc` path choice (API >= 9
    /// uses the `ets/` prefix).
    pub compile_api_version: u32,
    /// Bundle name from `AppScope/app.json5`.
    ///
    /// Used in `pack.info` `summary.app.bundleName`.
    pub bundle_name: String,
    /// Bundle version code from `AppScope/app.json5`.
    ///
    /// Used in `pack.info` `summary.app.version.code`.
    pub bundle_version_code: u64,
    /// Bundle version name from `AppScope/app.json5`.
    ///
    /// Used in `pack.info` `summary.app.version.name`.
    pub bundle_version_name: String,
    /// Optional build version emitted as `summary.app.version.build`.
    pub bundle_build_version: Option<String>,
    /// Bundle type string: `"app"` for HAP, `"shared"` for HSP.
    ///
    /// Used in `pack.info` `summary.app.bundleType`.
    pub bundle_type: String,
    /// Device types from the module manifest (e.g. `["phone", "tablet"]`).
    ///
    /// Defaults to `["phone"]` when empty.  Used in `pack.info`
    /// `summary.modules[].deviceType`.
    pub device_types: Vec<String>,
    /// Whether this module is installation-free (atomicService bundles).
    ///
    /// Used in `pack.info` `summary.modules[].distro.installationFree`.
    pub installation_free: bool,
    /// Whether this module is delivered with install.
    ///
    /// Used in `pack.info` `summary.modules[].distro.deliveryWithInstall` and
    /// `packages[].deliveryWithInstall`.
    pub delivery_with_install: bool,
    /// Tool mode: in-process (default) or subprocess delegation.
    pub tool_mode: PackageToolMode,
}

impl PackagePlan {
    /// Validate the plan before packaging begins.
    ///
    /// Checks that all **required** entries have source files that exist on
    /// disk.  Optional entries and `Generated` entries are not checked here;
    /// the packager handles them at write time.
    ///
    /// Returns `Err(PackError::MissingInput)` for the first missing required
    /// file, naming the archive path of the missing entry.
    ///
    /// hvigor upstream: `PackageHap.precheck()` / `PackageHar.precheck()` —
    /// the `beforeAlwaysAction` hook that validates task inputs before the
    /// main action runs.
    pub fn validate(&self) -> Result<(), PackError> {
        for entry in &self.entries {
            if !entry.required {
                continue;
            }
            match &entry.source {
                PackageEntrySource::File(path) => {
                    if !path.exists() {
                        return Err(PackError::MissingInput(format!(
                            "{} (source: {})",
                            entry.archive_path,
                            path.display()
                        )));
                    }
                }
                PackageEntrySource::Directory { root, .. } => {
                    if !root.exists() {
                        return Err(PackError::MissingInput(format!(
                            "{} (source dir: {})",
                            entry.archive_path,
                            root.display()
                        )));
                    }
                }
                // Generated entries are always considered present.
                PackageEntrySource::Generated => {}
            }
        }
        Ok(())
    }

    /// Returns the entries in the stable archive-write order.
    ///
    /// This is the order in which entries are written to the zip archive.
    /// The order matches hvigor's `PackageHap` / `PackageHar` write sequence
    /// to produce byte-for-byte reproducible archives.
    ///
    /// Currently returns entries in declaration order (the build-service is responsible
    /// for inserting them in the correct sequence).  A future refactor may sort
    /// entries by a stable `PackEntryId` key.
    pub fn ordered_entries(&self) -> &[PackageEntry] {
        &self.entries
    }

    /// Returns the path where the **unsigned** output archive will be written.
    ///
    /// For HAP/HSP this is the pre-signing artifact path that `arkdown-sign`
    /// reads.  For HAR this is the final artifact path (HAR is not signed).
    ///
    /// hvigor upstream: `PackageHap` output → `SignHap` input path.
    pub fn unsigned_output(&self) -> PackageOutput {
        PackageOutput::new(self.output_path.clone(), self.kind)
    }

    /// Returns the path where `pack.info` will be written inside the archive.
    ///
    /// Returns `None` for HAR archives, which do not contain `pack.info`.
    ///
    /// hvigor upstream: `make-pack-info.js` — the `MakePackInfo` task writes
    /// `pack.info` at the archive root for HAP/HSP.
    pub fn pack_info_path(&self) -> Option<&'static str> {
        if self.kind.needs_pack_info() {
            Some("pack.info")
        } else {
            None
        }
    }

    /// Returns the effective device types list, defaulting to `["phone"]` when
    /// the plan's `device_types` is empty.
    ///
    /// hvigor upstream: `pack.info` `deviceType` field default.
    pub fn effective_device_types(&self) -> Vec<String> {
        if self.device_types.is_empty() {
            vec!["phone".to_string()]
        } else {
            self.device_types.clone()
        }
    }

    /// Returns the archive path for `modules.abc`, accounting for the API
    /// version.
    ///
    /// - API >= 9: `"ets/modules.abc"`
    /// - API < 9: `"modules.abc"`
    ///
    /// hvigor upstream: `PackageHap` ABC path selection based on
    /// `compileSdkVersion`.
    pub fn abc_archive_path(&self) -> &'static str {
        if self.compile_api_version >= 9 {
            "ets/modules.abc"
        } else {
            "modules.abc"
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn make_plan(kind: PackageKind) -> PackagePlan {
        PackagePlan {
            module_name: "entry".to_string(),
            kind,
            output_path: PathBuf::from("/tmp/entry.hap"),
            entries: vec![],
            compile_api_version: 12,
            bundle_name: "com.example.app".to_string(),
            bundle_version_code: 1_000_000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_build_version: None,
            bundle_type: "app".to_string(),
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
            tool_mode: PackageToolMode::InProcess,
        }
    }

    // --- PackageKind ---

    #[test]
    fn test_hap_needs_pack_info() {
        assert!(PackageKind::Hap.needs_pack_info());
        assert!(PackageKind::Hsp.needs_pack_info());
        assert!(!PackageKind::Har.needs_pack_info());
    }

    #[test]
    fn test_hap_needs_abc() {
        assert!(PackageKind::Hap.needs_abc());
        assert!(PackageKind::Hsp.needs_abc());
        assert!(!PackageKind::Har.needs_abc());
    }

    #[test]
    fn test_kind_extension() {
        assert_eq!(PackageKind::Hap.extension(), "hap");
        assert_eq!(PackageKind::Hsp.extension(), "hsp");
        assert_eq!(PackageKind::Har.extension(), "har");
    }

    // --- PackageToolMode ---

    #[test]
    fn test_tool_mode_default_is_in_process() {
        assert_eq!(PackageToolMode::default(), PackageToolMode::InProcess);
    }

    #[test]
    fn test_tool_mode_subprocess_stores_path() {
        let mode = PackageToolMode::Subprocess {
            tool_path: PathBuf::from("/sdk/bin/packing-tool"),
        };
        match mode {
            PackageToolMode::Subprocess { tool_path } => {
                assert_eq!(tool_path, PathBuf::from("/sdk/bin/packing-tool"));
            }
            _ => panic!("expected Subprocess"),
        }
    }

    // --- PackageEntry constructors ---

    #[test]
    fn test_required_file_entry() {
        let entry = PackageEntry::required_file("module.json", PathBuf::from("/tmp/module.json"));
        assert_eq!(entry.archive_path, "module.json");
        assert!(entry.required);
        assert!(matches!(entry.source, PackageEntrySource::File(_)));
    }

    #[test]
    fn test_optional_file_entry() {
        let entry =
            PackageEntry::optional_file("ets/sourceMaps.map", PathBuf::from("/tmp/sourceMaps.map"));
        assert_eq!(entry.archive_path, "ets/sourceMaps.map");
        assert!(!entry.required);
    }

    #[test]
    fn test_directory_entry() {
        let entry = PackageEntry::directory("resources/", PathBuf::from("/tmp/resources"));
        assert_eq!(entry.archive_path, "resources/");
        assert!(!entry.required);
        match &entry.source {
            PackageEntrySource::Directory {
                root,
                archive_prefix,
            } => {
                assert_eq!(root, &PathBuf::from("/tmp/resources"));
                assert_eq!(archive_prefix, "resources/");
            }
            _ => panic!("expected Directory source"),
        }
    }

    #[test]
    fn test_generated_entry() {
        let entry = PackageEntry::generated("pack.info");
        assert_eq!(entry.archive_path, "pack.info");
        assert!(!entry.required);
        assert!(matches!(entry.source, PackageEntrySource::Generated));
    }

    // --- PackagePlan::validate ---

    #[test]
    fn test_validate_empty_plan_ok() {
        let plan = make_plan(PackageKind::Hap);
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_validate_required_file_missing_errors() {
        let mut plan = make_plan(PackageKind::Hap);
        plan.entries.push(PackageEntry::required_file(
            "module.json",
            PathBuf::from("/nonexistent/module.json"),
        ));
        let err = plan.validate().unwrap_err();
        assert!(
            matches!(&err, PackError::MissingInput(msg) if msg.contains("module.json")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_validate_optional_file_missing_ok() {
        let mut plan = make_plan(PackageKind::Hap);
        plan.entries.push(PackageEntry::optional_file(
            "ets/sourceMaps.map",
            PathBuf::from("/nonexistent/sourceMaps.map"),
        ));
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_validate_required_file_present_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("module.json");
        std::fs::write(&file, b"{}").unwrap();

        let mut plan = make_plan(PackageKind::Hap);
        plan.entries
            .push(PackageEntry::required_file("module.json", file));
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_validate_generated_entry_always_ok() {
        let mut plan = make_plan(PackageKind::Hap);
        // Generated entries have no on-disk source; validate must not error.
        let mut entry = PackageEntry::generated("pack.info");
        entry.required = true; // even when marked required
        plan.entries.push(entry);
        assert!(plan.validate().is_ok());
    }

    // --- PackagePlan::ordered_entries ---

    #[test]
    fn test_ordered_entries_preserves_insertion_order() {
        let mut plan = make_plan(PackageKind::Hap);
        plan.entries.push(PackageEntry::generated("module.json"));
        plan.entries
            .push(PackageEntry::generated("resources.index"));
        plan.entries
            .push(PackageEntry::generated("ets/modules.abc"));
        plan.entries.push(PackageEntry::generated("pack.info"));

        let names: Vec<&str> = plan
            .ordered_entries()
            .iter()
            .map(|e| e.archive_path.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "module.json",
                "resources.index",
                "ets/modules.abc",
                "pack.info"
            ]
        );
    }

    // --- PackagePlan::unsigned_output ---

    #[test]
    fn test_unsigned_output_path_and_kind() {
        let plan = make_plan(PackageKind::Hap);
        let out = plan.unsigned_output();
        assert_eq!(out.path, PathBuf::from("/tmp/entry.hap"));
        assert_eq!(out.kind, PackageKind::Hap);
        assert!(!out.signed);
    }

    // --- PackagePlan::pack_info_path ---

    #[test]
    fn test_pack_info_path_hap_hsp() {
        assert_eq!(
            make_plan(PackageKind::Hap).pack_info_path(),
            Some("pack.info")
        );
        let mut hsp = make_plan(PackageKind::Hsp);
        hsp.kind = PackageKind::Hsp;
        assert_eq!(hsp.pack_info_path(), Some("pack.info"));
    }

    #[test]
    fn test_pack_info_path_har_is_none() {
        assert_eq!(make_plan(PackageKind::Har).pack_info_path(), None);
    }

    // --- PackagePlan helpers ---

    #[test]
    fn test_effective_device_types_defaults_to_phone() {
        let plan = make_plan(PackageKind::Hap);
        assert_eq!(plan.effective_device_types(), vec!["phone"]);
    }

    #[test]
    fn test_effective_device_types_uses_plan_value() {
        let mut plan = make_plan(PackageKind::Hap);
        plan.device_types = vec!["phone".to_string(), "tablet".to_string()];
        assert_eq!(plan.effective_device_types(), vec!["phone", "tablet"]);
    }

    #[test]
    fn test_abc_archive_path_api9_and_above() {
        let plan = make_plan(PackageKind::Hap); // compile_api_version = 12
        assert_eq!(plan.abc_archive_path(), "ets/modules.abc");
    }

    #[test]
    fn test_abc_archive_path_below_api9() {
        let mut plan = make_plan(PackageKind::Hap);
        plan.compile_api_version = 8;
        assert_eq!(plan.abc_archive_path(), "modules.abc");
    }

    // --- PackageOutput ---

    #[test]
    fn test_package_output_new_is_unsigned() {
        let out = PackageOutput::new(PathBuf::from("/out/entry.hap"), PackageKind::Hap);
        assert_eq!(out.path, PathBuf::from("/out/entry.hap"));
        assert_eq!(out.kind, PackageKind::Hap);
        assert!(!out.signed);
    }
}
