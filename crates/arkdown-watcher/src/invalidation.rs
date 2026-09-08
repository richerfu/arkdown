//! Invalidation event classification.
//!
//! Maps raw filesystem `WatchEvent`s to typed `InvalidationEvent`s that
//! describe which build stages must be re-run and which modules are affected.
//!
//! Upstream: `invalid-cache-plugin.js`, rollup watch cache.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use arkdown_path::classify::is_inside_oh_modules;

use crate::watcher::WatchEvent;

// ---------------------------------------------------------------------------
// AffectedStage
// ---------------------------------------------------------------------------

/// A build stage that must be re-run as a result of a file change.
///
/// Variants are ordered roughly by pipeline position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AffectedStage {
    /// Re-run `module.json5` / `build-profile.json5` processing.
    Profile,
    /// Re-run `restool` / resource compilation.
    Resource,
    /// Re-run the rolldown bundle step.
    Bundle,
    /// Re-run `es2abc` / ABC compilation.
    Ts2abc,
    /// Re-assemble the HAP/HSP/HAR zip.
    Package,
    /// Re-sign the artifact.
    Sign,
    /// Re-run the entire pipeline.
    All,
}

impl AffectedStage {
    /// Human-readable name for diagnostic output.
    pub fn as_str(&self) -> &'static str {
        match self {
            AffectedStage::Profile => "profile",
            AffectedStage::Resource => "resource",
            AffectedStage::Bundle => "bundle",
            AffectedStage::Ts2abc => "ts2abc",
            AffectedStage::Package => "package",
            AffectedStage::Sign => "sign",
            AffectedStage::All => "all",
        }
    }
}

// ---------------------------------------------------------------------------
// InvalidationReason
// ---------------------------------------------------------------------------

/// Why a build invalidation was triggered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidationReason {
    /// An ETS/TS/JS source file changed.
    SourceFileChanged {
        /// Absolute path of the changed file.
        path: PathBuf,
    },
    /// A `module.json5` or `build-profile.json5` file changed.
    ProfileFileChanged {
        /// Absolute path of the changed profile file.
        path: PathBuf,
    },
    /// A resource file (under `src/main/resources/`) changed.
    ResourceChanged {
        /// Absolute path of the changed resource file.
        path: PathBuf,
    },
    /// The SDK or toolchain changed (e.g. `OHOS_SDK_HOME` re-pointed).
    SdkChanged,
    /// An `app.json5` or `pack.info` manifest changed.
    ManifestChanged {
        /// Absolute path of the changed manifest file.
        path: PathBuf,
    },
    /// A dependency in `oh_modules/` or `oh-package.json5` changed.
    DependencyChanged,
}

impl InvalidationReason {
    /// Return a short machine-readable tag for this reason.
    pub fn tag(&self) -> &'static str {
        match self {
            InvalidationReason::SourceFileChanged { .. } => "source_changed",
            InvalidationReason::ProfileFileChanged { .. } => "profile_changed",
            InvalidationReason::ResourceChanged { .. } => "resource_changed",
            InvalidationReason::SdkChanged => "sdk_changed",
            InvalidationReason::ManifestChanged { .. } => "manifest_changed",
            InvalidationReason::DependencyChanged => "dependency_changed",
        }
    }

    /// Return the affected path if this reason is path-based.
    pub fn path(&self) -> Option<&Path> {
        match self {
            InvalidationReason::SourceFileChanged { path }
            | InvalidationReason::ProfileFileChanged { path }
            | InvalidationReason::ResourceChanged { path }
            | InvalidationReason::ManifestChanged { path } => Some(path),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// AffectedModuleSet
// ---------------------------------------------------------------------------

/// The set of OHOS modules that need to be rebuilt.
#[derive(Debug, Clone, Default)]
pub struct AffectedModuleSet {
    /// Names of the affected modules (e.g. `["entry", "feature"]`).
    pub module_names: Vec<String>,
    /// When `true`, all modules in the project are affected (e.g. SDK change).
    pub is_global: bool,
}

impl AffectedModuleSet {
    /// Create an empty (no-op) module set.
    pub fn empty() -> Self {
        AffectedModuleSet::default()
    }

    /// Create a global set that affects all modules.
    pub fn global() -> Self {
        AffectedModuleSet {
            module_names: vec![],
            is_global: true,
        }
    }

    /// Create a set for a single named module.
    pub fn single(name: impl Into<String>) -> Self {
        AffectedModuleSet {
            module_names: vec![name.into()],
            is_global: false,
        }
    }

    /// Create a set for multiple named modules.
    pub fn many(names: Vec<String>) -> Self {
        AffectedModuleSet {
            module_names: names,
            is_global: false,
        }
    }

    /// Return `true` if this set is non-empty or global.
    pub fn is_nonempty(&self) -> bool {
        self.is_global || !self.module_names.is_empty()
    }
}

// ---------------------------------------------------------------------------
// InvalidationEvent
// ---------------------------------------------------------------------------

/// A classified build-invalidation event derived from a filesystem change.
///
/// Upstream: `invalid-cache-plugin.js`, rollup watch cache.
#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    /// Why the invalidation was triggered.
    pub reason: InvalidationReason,
    /// Which build stages must be re-run.
    pub affected_stages: Vec<AffectedStage>,
    /// Which modules are affected.
    pub affected_modules_set: AffectedModuleSet,
}

/// One debounced, typed filesystem invalidation batch.
///
/// The exact changed paths are retained for hvigor-compatible
/// `shouldInvalidCache` hooks and the batch exposes whether a source-only hot
/// rebuild is valid.
#[derive(Debug, Clone, Default)]
pub struct WatchInvalidationBatch {
    /// Classified invalidation events.
    pub events: Vec<InvalidationEvent>,
    /// Actionable raw watcher events, preserving event kind and arrival order.
    ///
    /// Hvigor's `watchChangedFiles` Rollup plugin distinguishes `update` from
    /// `create`/`delete`, so `changed_files` alone is not sufficient for its
    /// hot-reload handoff.
    pub raw_events: Vec<WatchEvent>,
    /// Deduplicated changed files passed to cache/plugin invalidation.
    pub changed_files: Vec<PathBuf>,
}

impl WatchInvalidationBatch {
    /// Classify a debounced watcher batch and discard non-actionable events.
    pub fn from_watch_events(
        events: &[WatchEvent],
        fingerprint_baseline: &HashMap<PathBuf, u64>,
    ) -> Self {
        let mut invalidations = Vec::new();
        let mut raw_events = Vec::new();
        let mut changed_files = BTreeSet::new();
        for event in events {
            if let Some(invalidation) =
                InvalidationEvent::from_watch_event(event, fingerprint_baseline)
            {
                changed_files.insert(event.path.clone());
                raw_events.push(event.clone());
                invalidations.push(invalidation);
            }
        }
        Self {
            events: invalidations,
            raw_events,
            changed_files: changed_files.into_iter().collect(),
        }
    }

    /// Return whether the batch contains no actionable changes.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Classify this batch as source-only hot or full-pipeline cold.
    pub fn change_kind(&self) -> WatchChangeKind {
        if !self.events.is_empty()
            && self
                .events
                .iter()
                .all(|event| matches!(event.reason, InvalidationReason::SourceFileChanged { .. }))
        {
            WatchChangeKind::Hot
        } else {
            WatchChangeKind::Cold
        }
    }
}

impl InvalidationEvent {
    /// Classify a raw `WatchEvent` into an `InvalidationEvent`.
    ///
    /// Returns `None` when:
    /// - The file extension is not actionable.
    /// - The fingerprint has not actually changed (content-identical write).
    ///
    /// `fingerprint_baseline` maps file paths to their last-known FNV-1a
    /// fingerprint (provided by the build service from `BuildSession`).
    pub fn from_file_event(
        path: &Path,
        fingerprint_baseline: &HashMap<PathBuf, u64>,
    ) -> Option<Self> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Classify by path pattern first, then by extension.
        if is_resource_path(path) {
            return Some(InvalidationEvent {
                reason: InvalidationReason::ResourceChanged {
                    path: path.to_path_buf(),
                },
                affected_stages: vec![AffectedStage::Resource, AffectedStage::Package],
                affected_modules_set: AffectedModuleSet::empty(),
            });
        }

        if is_profile_path(path) {
            return Some(InvalidationEvent {
                reason: InvalidationReason::ProfileFileChanged {
                    path: path.to_path_buf(),
                },
                affected_stages: vec![
                    AffectedStage::Profile,
                    AffectedStage::Bundle,
                    AffectedStage::Package,
                ],
                affected_modules_set: AffectedModuleSet::empty(),
            });
        }

        if is_manifest_path(path) {
            return Some(InvalidationEvent {
                reason: InvalidationReason::ManifestChanged {
                    path: path.to_path_buf(),
                },
                affected_stages: vec![AffectedStage::All],
                affected_modules_set: AffectedModuleSet::global(),
            });
        }

        if is_dependency_path(path) {
            return Some(InvalidationEvent {
                reason: InvalidationReason::DependencyChanged,
                affected_stages: vec![AffectedStage::All],
                affected_modules_set: AffectedModuleSet::global(),
            });
        }

        if is_source_ext(ext) {
            // Check fingerprint to avoid spurious rebuilds from content-identical writes.
            if let Some(&baseline) = fingerprint_baseline.get(path) {
                if let Ok(current) = compute_file_fingerprint(path) {
                    if !Self::match_fingerprint(path, current, baseline) {
                        return None;
                    }
                }
            }
            return Some(InvalidationEvent {
                reason: InvalidationReason::SourceFileChanged {
                    path: path.to_path_buf(),
                },
                affected_stages: vec![
                    AffectedStage::Bundle,
                    AffectedStage::Ts2abc,
                    AffectedStage::Package,
                ],
                affected_modules_set: AffectedModuleSet::empty(),
            });
        }

        None
    }

    /// Construct an `InvalidationEvent` directly from a `WatchEvent`.
    ///
    /// Convenience wrapper around `from_file_event`.
    pub fn from_watch_event(
        event: &WatchEvent,
        fingerprint_baseline: &HashMap<PathBuf, u64>,
    ) -> Option<Self> {
        Self::from_file_event(&event.path, fingerprint_baseline)
    }

    /// Return `true` if the file fingerprint has actually changed.
    ///
    /// Used to suppress rebuilds triggered by content-identical writes (e.g.
    /// editor auto-save that writes the same bytes).
    pub fn match_fingerprint(_path: &Path, current: u64, baseline: u64) -> bool {
        current != baseline
    }

    /// Return the affected module set for this event.
    pub fn affected_modules(&self) -> &AffectedModuleSet {
        &self.affected_modules_set
    }

    /// Return a human-readable explanation of this invalidation event.
    ///
    /// Suitable for `tracing::info!` and diagnostic output.
    pub fn explain(&self) -> String {
        let stages: Vec<&str> = self.affected_stages.iter().map(|s| s.as_str()).collect();
        let modules = if self.affected_modules_set.is_global {
            "all modules".to_string()
        } else if self.affected_modules_set.module_names.is_empty() {
            "unknown module".to_string()
        } else {
            self.affected_modules_set.module_names.join(", ")
        };

        match &self.reason {
            InvalidationReason::SourceFileChanged { path } => format!(
                "source file changed: {} → stages [{}] in {}",
                path.display(),
                stages.join(", "),
                modules
            ),
            InvalidationReason::ProfileFileChanged { path } => format!(
                "profile file changed: {} → stages [{}] in {}",
                path.display(),
                stages.join(", "),
                modules
            ),
            InvalidationReason::ResourceChanged { path } => format!(
                "resource changed: {} → stages [{}] in {}",
                path.display(),
                stages.join(", "),
                modules
            ),
            InvalidationReason::SdkChanged => format!(
                "SDK changed → stages [{}] in {}",
                stages.join(", "),
                modules
            ),
            InvalidationReason::ManifestChanged { path } => format!(
                "manifest changed: {} → stages [{}] in {}",
                path.display(),
                stages.join(", "),
                modules
            ),
            InvalidationReason::DependencyChanged => format!(
                "dependency changed → stages [{}] in {}",
                stages.join(", "),
                modules
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Path classification helpers (private)
// ---------------------------------------------------------------------------

fn is_resource_path(path: &Path) -> bool {
    let mut components = path.components().map(|component| component.as_os_str());
    while let Some(component) = components.next() {
        if component != "src" {
            continue;
        }
        if components.next() == Some(std::ffi::OsStr::new("main"))
            && components.next() == Some(std::ffi::OsStr::new("resources"))
        {
            return true;
        }
    }
    false
}

fn is_profile_path(path: &Path) -> bool {
    // build-profile.json5, module.json5, hvigorfile.ts, etc.
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(
            "build-profile.json5"
                | "module.json5"
                | "hvigorfile.ts"
                | "hvigorfile.js"
                | "loader.json"
        )
    )
}

fn is_manifest_path(path: &Path) -> bool {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(filename, "app.json5" | "pack.info" | "pkgContextInfo.json")
}

fn is_dependency_path(path: &Path) -> bool {
    is_inside_oh_modules(path) || {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        matches!(filename, "oh-package.json5" | "oh-package-lock.json5")
    }
}

fn is_source_ext(ext: &str) -> bool {
    matches!(
        ext,
        "ets" | "ts" | "js" | "mjs" | "cjs" | "json" | "json5" | "visual"
    )
}

/// Compute a simple FNV-1a fingerprint for a file.
///
/// Returns `Err` if the file cannot be read (deleted, permission denied, etc.).
fn compute_file_fingerprint(path: &Path) -> Result<u64, std::io::Error> {
    let bytes = std::fs::read(path)?;
    Ok(arkdown_core::fnv1a64(&bytes))
}

// ---------------------------------------------------------------------------
// Hot/cold watch change classification
// ---------------------------------------------------------------------------

/// Whether a file change requires only a bundle re-run (hot) or a full
/// pipeline rebuild (cold).
///
/// # Upstream correspondence
///
/// Mirrors hvigor's `changedFiles` + `event_name` + importer-graph logic
/// used by `arkts-pack.ts` to decide between incremental and full rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchChangeKind {
    /// ETS/TS/JS source change — only the bundle stage needs to rerun.
    ///
    /// Corresponds to hvigor's "hot" incremental rebuild path where only
    /// module-graph entries that import the changed file are re-bundled.
    Hot,
    /// Config or resource change — full pipeline rebuild required.
    ///
    /// Triggered by `module.json5`, `app.json5`, `build-profile.json5`,
    /// `oh-package.json5`, or resource files under `src/main/resources/`.
    Cold,
}

/// Classify a changed file path as `Hot` (bundle-only) or `Cold` (full rebuild).
///
/// # Arguments
///
/// * `path` — Absolute path of the changed file.
/// * `source_roots` — Module source roots (`src/main` directories) to check
///   membership against.  Any ETS/TS/JS file under a source root is hot.
///
/// # Logic
///
/// - File has a source extension (`.ets`, `.ts`, `.js`, `.d.ts`, `.d.ets`)
///   **and** is under a known source root → `Hot`.
/// - Otherwise → `Cold`.
///
/// Files under `oh_modules/` are treated as `Cold` because they represent
/// dependency changes that may affect the resolver.
pub fn classify_watch_change(path: &Path, source_roots: &[&Path]) -> WatchChangeKind {
    // Dependency changes are always cold.
    if is_inside_oh_modules(path) {
        return WatchChangeKind::Cold;
    }

    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Config files trigger a cold rebuild regardless of location.
    let cold_names = [
        "module.json5",
        "app.json5",
        "build-profile.json5",
        "oh-package.json5",
        "hvigorfile.ts",
        "hvigorfile.js",
        "hvigorfile.cjs",
        "hvigorfile.mjs",
        "loader.json",
    ];
    if cold_names.contains(&file_name) {
        return WatchChangeKind::Cold;
    }

    // Resource files are cold.
    if is_resource_path(path) {
        return WatchChangeKind::Cold;
    }

    // ETS/TS/JS source files under a known source root are hot.
    let hot_extensions = ["ets", "ts", "js", "d.ts", "d.ets"];
    let has_hot_ext = hot_extensions
        .iter()
        .any(|ext| file_name.ends_with(&format!(".{ext}")));

    if has_hot_ext && source_roots.iter().any(|root| path.starts_with(root)) {
        return WatchChangeKind::Hot;
    }

    WatchChangeKind::Cold
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_file_change_produces_bundle_ts2abc_package_stages() {
        let path = PathBuf::from("/project/entry/src/main/ets/pages/Index.ets");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline)
            .expect("ets file must produce event");
        assert_eq!(event.reason.tag(), "source_changed");
        assert!(event.affected_stages.contains(&AffectedStage::Bundle));
        assert!(event.affected_stages.contains(&AffectedStage::Ts2abc));
        assert!(event.affected_stages.contains(&AffectedStage::Package));
    }

    #[test]
    fn test_resource_change_produces_resource_package_stages() {
        let path = PathBuf::from("/project/entry/src/main/resources/base/media/icon.png");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline)
            .expect("resource file must produce event");
        assert_eq!(event.reason.tag(), "resource_changed");
        assert!(event.affected_stages.contains(&AffectedStage::Resource));
        assert!(event.affected_stages.contains(&AffectedStage::Package));
    }

    #[test]
    fn test_profile_change_produces_profile_bundle_package_stages() {
        let path = PathBuf::from("/project/entry/build-profile.json5");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline)
            .expect("profile file must produce event");
        assert_eq!(event.reason.tag(), "profile_changed");
        assert!(event.affected_stages.contains(&AffectedStage::Profile));
        assert!(event.affected_stages.contains(&AffectedStage::Bundle));
    }

    #[test]
    fn test_build_profile_like_json5_is_not_profile_change() {
        let path = PathBuf::from("/project/entry/not-build-profile.json5");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline)
            .expect("json5 source file must produce event");
        assert_eq!(event.reason.tag(), "source_changed");
        assert!(!event.affected_stages.contains(&AffectedStage::Profile));
    }

    #[test]
    fn test_manifest_change_produces_all_stage_and_global_modules() {
        let path = PathBuf::from("/project/AppScope/app.json5");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline)
            .expect("manifest file must produce event");
        assert_eq!(event.reason.tag(), "manifest_changed");
        assert!(event.affected_stages.contains(&AffectedStage::All));
        assert!(event.affected_modules_set.is_global);
    }

    #[test]
    fn test_dependency_change_produces_all_stage() {
        let path = PathBuf::from("/project/entry/oh-package.json5");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline)
            .expect("oh-package.json5 must produce event");
        assert_eq!(event.reason.tag(), "dependency_changed");
        assert!(event.affected_stages.contains(&AffectedStage::All));
    }

    #[test]
    fn test_unknown_extension_returns_none() {
        let path = PathBuf::from("/project/entry/src/main/ets/pages/Index.png");
        let baseline = HashMap::new();
        // .png in a non-resource path should return None
        let event = InvalidationEvent::from_file_event(&path, &baseline);
        assert!(event.is_none(), "png in non-resource path must return None");
    }

    #[test]
    fn visual_change_invalidates_bundle_bytecode_and_package() {
        let path = PathBuf::from("/project/entry/src/main/supervisual/pages/Index.visual");
        let event = InvalidationEvent::from_file_event(&path, &HashMap::new())
            .expect("visual input must produce an invalidation");

        assert!(matches!(
            event.reason,
            InvalidationReason::SourceFileChanged { .. }
        ));
        assert_eq!(
            event.affected_stages,
            vec![
                AffectedStage::Bundle,
                AffectedStage::Ts2abc,
                AffectedStage::Package,
            ]
        );
    }

    #[test]
    fn watch_batch_preserves_changed_files_and_cold_precedence() {
        let source = WatchEvent::new(
            PathBuf::from("/project/entry/src/main/ets/Index.ets"),
            crate::watcher::WatchEventKind::Modified,
        );
        let resource = WatchEvent::new(
            PathBuf::from("/project/entry/src/main/resources/base/media/icon.png"),
            crate::watcher::WatchEventKind::Modified,
        );
        let batch = WatchInvalidationBatch::from_watch_events(&[source, resource], &HashMap::new());
        assert_eq!(batch.changed_files.len(), 2);
        assert_eq!(batch.change_kind(), WatchChangeKind::Cold);
    }

    #[test]
    fn test_match_fingerprint_changed_returns_true() {
        let path = Path::new("/tmp/test.ets");
        assert!(InvalidationEvent::match_fingerprint(path, 100, 200));
    }

    #[test]
    fn test_match_fingerprint_unchanged_returns_false() {
        let path = Path::new("/tmp/test.ets");
        assert!(!InvalidationEvent::match_fingerprint(path, 100, 100));
    }

    #[test]
    fn test_explain_source_changed() {
        let path = PathBuf::from("/project/entry/src/main/ets/Index.ets");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline).unwrap();
        let explanation = event.explain();
        assert!(explanation.contains("source file changed"));
        assert!(explanation.contains("Index.ets"));
        assert!(explanation.contains("bundle"));
    }

    #[test]
    fn test_explain_resource_changed() {
        let path = PathBuf::from("/project/entry/src/main/resources/base/media/icon.png");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline).unwrap();
        let explanation = event.explain();
        assert!(explanation.contains("resource changed"));
        assert!(explanation.contains("icon.png"));
    }

    #[test]
    fn test_affected_modules_set_global() {
        let set = AffectedModuleSet::global();
        assert!(set.is_global);
        assert!(set.is_nonempty());
    }

    #[test]
    fn test_affected_modules_set_single() {
        let set = AffectedModuleSet::single("entry");
        assert!(!set.is_global);
        assert_eq!(set.module_names, vec!["entry"]);
        assert!(set.is_nonempty());
    }

    #[test]
    fn test_affected_modules_set_empty() {
        let set = AffectedModuleSet::empty();
        assert!(!set.is_global);
        assert!(!set.is_nonempty());
    }

    #[test]
    fn test_affected_stage_as_str() {
        assert_eq!(AffectedStage::Profile.as_str(), "profile");
        assert_eq!(AffectedStage::Resource.as_str(), "resource");
        assert_eq!(AffectedStage::Bundle.as_str(), "bundle");
        assert_eq!(AffectedStage::Ts2abc.as_str(), "ts2abc");
        assert_eq!(AffectedStage::Package.as_str(), "package");
        assert_eq!(AffectedStage::Sign.as_str(), "sign");
        assert_eq!(AffectedStage::All.as_str(), "all");
    }

    #[test]
    fn test_oh_modules_path_triggers_dependency_change() {
        let path = PathBuf::from("/project/entry/oh_modules/@ohos/net/index.ets");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline)
            .expect("oh_modules path must produce event");
        assert_eq!(event.reason.tag(), "dependency_changed");
    }

    #[test]
    fn test_loader_json_triggers_profile_change() {
        let path = PathBuf::from("/project/entry/build/loader.json");
        let baseline = HashMap::new();
        let event = InvalidationEvent::from_file_event(&path, &baseline)
            .expect("loader.json must produce event");
        assert_eq!(event.reason.tag(), "profile_changed");
    }

    #[test]
    fn test_invalidation_reason_path_accessor() {
        let path = PathBuf::from("/project/entry/src/main/ets/Index.ets");
        let reason = InvalidationReason::SourceFileChanged { path: path.clone() };
        assert_eq!(reason.path(), Some(path.as_path()));

        let reason2 = InvalidationReason::SdkChanged;
        assert_eq!(reason2.path(), None);
    }

    // --- WatchChangeKind tests ---

    #[test]
    fn test_ets_under_source_root_is_hot() {
        let root = PathBuf::from("/project/entry/src/main");
        let path = PathBuf::from("/project/entry/src/main/ets/pages/Index.ets");
        let roots = [root.as_path()];
        assert_eq!(classify_watch_change(&path, &roots), WatchChangeKind::Hot);
    }

    #[test]
    fn test_ts_under_source_root_is_hot() {
        let root = PathBuf::from("/project/entry/src/main");
        let path = PathBuf::from("/project/entry/src/main/ets/utils/Helper.ts");
        assert_eq!(
            classify_watch_change(&path, &[root.as_path()]),
            WatchChangeKind::Hot
        );
    }

    #[test]
    fn test_module_json5_is_cold() {
        let root = PathBuf::from("/project/entry/src/main");
        let path = PathBuf::from("/project/entry/src/main/module.json5");
        assert_eq!(
            classify_watch_change(&path, &[root.as_path()]),
            WatchChangeKind::Cold
        );
    }

    #[test]
    fn test_oh_package_json5_is_cold() {
        let root = PathBuf::from("/project/entry/src/main");
        let path = PathBuf::from("/project/entry/oh-package.json5");
        assert_eq!(
            classify_watch_change(&path, &[root.as_path()]),
            WatchChangeKind::Cold
        );
    }

    #[test]
    fn test_resource_file_is_cold() {
        let root = PathBuf::from("/project/entry/src/main");
        let path = PathBuf::from("/project/entry/src/main/resources/base/media/icon.png");
        assert_eq!(
            classify_watch_change(&path, &[root.as_path()]),
            WatchChangeKind::Cold
        );
    }

    #[test]
    fn test_ets_outside_source_root_is_cold() {
        let root = PathBuf::from("/project/entry/src/main");
        let path = PathBuf::from("/project/feature/src/main/ets/Index.ets");
        assert_eq!(
            classify_watch_change(&path, &[root.as_path()]),
            WatchChangeKind::Cold
        );
    }

    #[test]
    fn test_oh_modules_is_cold() {
        let root = PathBuf::from("/project/entry/src/main");
        let path = PathBuf::from("/project/entry/oh_modules/@ohos/net/index.ts");
        assert_eq!(
            classify_watch_change(&path, &[root.as_path()]),
            WatchChangeKind::Cold
        );
    }
}
