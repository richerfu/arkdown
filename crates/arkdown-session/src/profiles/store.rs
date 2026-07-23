//! [`ProfileStore`] — session-owned profile JSON/JSON5 store.
//!
//! `ProfileStore` is built once during session startup and provides cached,
//! indexed access to all profile-type files (router maps, startup configs,
//! share configs, privacy profiles, etc.) for all modules.
//!
//! Profile code no longer constructs profile paths manually — they call
//! typed accessors on `ProfileStore` and receive pre-discovered file refs.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `ProfileStore` | `ProfileStore` session-level file index |
//! | `ProfileFile` | one profile file entry (path + content cache) |
//! | `ParsedProfile` | typed profile content after JSON5 parse |
//! | `ProfileRef` | reference to a profile file for session lookup |
//! | `RouterMapInventory` | `ProcessRouterMap` input inventory |
//! | `StartupInventory` | `ProcessStartupConfig` input inventory |
//! | `ShareInventory` | `ProcessShareConfig` input inventory |

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use crate::fingerprint::{Fingerprint, FingerprintBuilder};
use crate::paths::ModulePathLayout;

/// Errors produced while discovering or parsing profile files.
#[derive(Debug, Error)]
pub enum ProfileStoreError {
    /// A profile file could not be read.
    #[error("failed to read profile file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A profile file could not be parsed as JSON5.
    #[error("failed to parse profile file {path}: {message}")]
    Parse { path: PathBuf, message: String },
}

// ---------------------------------------------------------------------------
// ProfileRef
// ---------------------------------------------------------------------------

/// A reference to a profile file for session lookup.
///
/// `ProfileRef` is a lightweight token that callers use to retrieve the
/// actual file content from the [`ProfileStore`].  It avoids passing
/// `PathBuf` values around and ensures all profile access goes through the
/// store.
///
/// # Upstream correspondence
///
/// Corresponds to the profile file reference used by `ProcessRouterMap`,
/// `ProcessShareConfig`, `ProcessStartupConfig` to locate their input files.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileRef {
    /// The absolute path to the profile file.
    pub path: PathBuf,
    /// The module name this profile belongs to.
    pub module_name: String,
    /// The kind of profile file.
    pub kind: ProfileFileKind,
}

/// The kind of profile file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProfileFileKind {
    /// `router_map.json` — router map entries.
    RouterMap,
    /// `startup_config.json` / `harStartup.json` — startup task config.
    StartupConfig,
    /// `share.json` — cross-app share config.
    ShareConfig,
    /// `privacy_statement.json` — privacy statement.
    Privacy,
    /// `module.json5` — module manifest.
    ModuleJson,
    /// `forms.json` / `shortcuts.json` / other profile.
    Other { name: String },
}

impl ProfileFileKind {
    /// Classify a profile file path by its canonical profile file name.
    pub fn from_path(path: &Path) -> Self {
        match path.file_name().and_then(|value| value.to_str()) {
            Some("router_map.json" | "router_map.json5") => Self::RouterMap,
            Some(
                "startup_config.json"
                | "startup_config.json5"
                | "appStartup.json"
                | "appStartup.json5"
                | "harStartup.json"
                | "harStartup.json5"
                | "hspStartup.json"
                | "hspStartup.json5",
            ) => Self::StartupConfig,
            Some("share.json" | "share.json5") => Self::ShareConfig,
            Some("privacy_statement.json" | "privacy_statement.json5") => Self::Privacy,
            Some("module.json5") => Self::ModuleJson,
            Some(name) => Self::Other {
                name: name.to_string(),
            },
            None => Self::Other {
                name: String::new(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// ProfileFile
// ---------------------------------------------------------------------------

/// A single profile file entry in the [`ProfileStore`].
///
/// # Upstream correspondence
///
/// Corresponds to one profile file input tracked by `ProfileStore` in hvigor.
#[derive(Debug, Clone)]
pub struct ProfileFile {
    /// The profile reference token.
    pub profile_ref: ProfileRef,
    /// Content fingerprint for cache invalidation.
    pub fingerprint: Fingerprint,
    /// Cached raw content (lazily populated).
    content: Option<Arc<String>>,
}

impl ProfileFile {
    /// Create a profile file entry from a path.
    pub fn from_path(
        path: impl Into<PathBuf>,
        module_name: impl Into<String>,
        kind: ProfileFileKind,
    ) -> Result<Self, ProfileStoreError> {
        let path = path.into();
        let fingerprint = Fingerprint::of_file(&path).map_err(|source| ProfileStoreError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(Self {
            profile_ref: ProfileRef {
                path,
                module_name: module_name.into(),
                kind,
            },
            fingerprint,
            content: None,
        })
    }

    /// Read the content of this profile file.
    pub fn read_content(&mut self) -> Result<Arc<String>, ProfileStoreError> {
        if let Some(content) = &self.content {
            return Ok(Arc::clone(content));
        }
        let text = std::fs::read_to_string(&self.profile_ref.path).map_err(|source| {
            ProfileStoreError::Io {
                path: self.profile_ref.path.clone(),
                source,
            }
        })?;
        let arc = Arc::new(text);
        self.content = Some(Arc::clone(&arc));
        Ok(arc)
    }
}

// ---------------------------------------------------------------------------
// ParsedProfile
// ---------------------------------------------------------------------------

/// Typed parsed profile content.
///
/// # Upstream correspondence
///
/// Corresponds to the parsed profile content used by `ProcessRouterMap`,
/// `ProcessStartupConfig`, `ProcessShareConfig`.
#[derive(Debug, Clone)]
pub struct ParsedProfile {
    /// The source file this was parsed from.
    pub source: ProfileRef,
    /// The parsed JSON value.
    pub content: serde_json::Value,
}

impl ParsedProfile {
    /// Parse a profile file.
    pub fn from_file(profile_file: &mut ProfileFile) -> Result<Self, ProfileStoreError> {
        let content = profile_file.read_content()?;
        let value: serde_json::Value =
            json5::from_str(&content).map_err(|error| ProfileStoreError::Parse {
                path: profile_file.profile_ref.path.clone(),
                message: error.to_string(),
            })?;
        Ok(Self {
            source: profile_file.profile_ref.clone(),
            content: value,
        })
    }
}

// ---------------------------------------------------------------------------
// RouterMapInventory
// ---------------------------------------------------------------------------

/// Inventory of router map files for a module.
///
/// # Upstream correspondence
///
/// Corresponds to `ProcessRouterMap` input — the set of `router_map.json`
/// files discovered for a module and its dependencies.
#[derive(Debug, Clone, Default)]
pub struct RouterMapInventory {
    /// Router map files for this module and its dependencies.
    pub files: Vec<ProfileFile>,
}

impl RouterMapInventory {
    /// Return the number of router map files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Return `true` if there are no router map files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

// ---------------------------------------------------------------------------
// StartupInventory
// ---------------------------------------------------------------------------

/// Inventory of startup config files for a module.
///
/// # Upstream correspondence
///
/// Corresponds to `ProcessStartupConfig` input — `startup_config.json` and
/// `harStartup.json` files for a module.
#[derive(Debug, Clone, Default)]
pub struct StartupInventory {
    /// Startup config files.
    pub files: Vec<ProfileFile>,
}

// ---------------------------------------------------------------------------
// ShareInventory
// ---------------------------------------------------------------------------

/// Inventory of share config files for a module.
///
/// # Upstream correspondence
///
/// Corresponds to `ProcessShareConfig` input — `share.json` files for a
/// module and its HAR dependencies.
#[derive(Debug, Clone, Default)]
pub struct ShareInventory {
    /// Share config files.
    pub files: Vec<ProfileFile>,
}

// ---------------------------------------------------------------------------
// ProfileStore
// ---------------------------------------------------------------------------

/// Session-owned profile file store.
///
/// `ProfileStore` is the single source of truth for all profile files in a
/// build session.  It is populated at session startup via [`discover`] and
/// provides typed accessors for each profile kind.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor's profile file discovery in `ProfileStore` combined
/// with the hvigor task inputs for `ProcessRouterMap`, `ProcessShareConfig`,
/// and `ProcessStartupConfig`.
#[derive(Debug, Clone, Default)]
pub struct ProfileStore {
    /// Base path used to serialize profile input keys for fingerprinting.
    base: PathBuf,
    /// All profile files indexed by absolute path.
    files: BTreeMap<PathBuf, ProfileFile>,
    /// Module-level router map inventories.
    router_maps: BTreeMap<String, RouterMapInventory>,
    /// Module-level startup inventories.
    startups: BTreeMap<String, StartupInventory>,
    /// Module-level share inventories.
    shares: BTreeMap<String, ShareInventory>,
}

impl ProfileStore {
    /// Discover and index profile files for every module in a project snapshot.
    pub fn discover_project(
        project: &crate::project::snapshot::ProjectSnapshot,
    ) -> Result<Self, ProfileStoreError> {
        let mut store = Self::new(&project.root);
        for (module_name, module) in &project.modules {
            store.scan_module(module_name, &module.root)?;
        }
        Ok(store)
    }

    /// Discover and index all profile files in `module_root`.
    ///
    /// Scans `src/main/resources/` and `src/main/ets/` for profile JSON5
    /// files and builds the internal index.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to hvigor's profile discovery during session startup.
    pub fn discover(module_name: &str, module_root: &Path) -> Result<Self, ProfileStoreError> {
        let mut store = Self::new(module_root);
        store.scan_module(module_name, module_root)?;
        Ok(store)
    }

    fn new(base: &Path) -> Self {
        Self {
            base: base.to_path_buf(),
            ..Self::default()
        }
    }

    /// Return a profile file by its `ProfileRef`.
    pub fn profile_by_ref(&self, ref_: &ProfileRef) -> Option<&ProfileFile> {
        self.files.get(&ref_.path)
    }

    /// Return the router map inventory for a module.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `ProcessRouterMap` input discovery.
    pub fn router_maps_for_module(&self, module_name: &str) -> &RouterMapInventory {
        self.router_maps
            .get(module_name)
            .unwrap_or(&EMPTY_ROUTER_MAP)
    }

    /// Return the startup config inventory for a module.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `ProcessStartupConfig` input discovery.
    pub fn startup_for_module(&self, module_name: &str) -> &StartupInventory {
        self.startups.get(module_name).unwrap_or(&EMPTY_STARTUP)
    }

    /// Return the share config inventory for a module.
    pub fn share_for_module(&self, module_name: &str) -> &ShareInventory {
        self.shares.get(module_name).unwrap_or(&EMPTY_SHARE)
    }

    /// Compute the combined fingerprint of all profile files.
    ///
    /// Used as part of the session's `InputFingerprintSet`.
    pub fn fingerprint(&self) -> Fingerprint {
        let mut builder = FingerprintBuilder::new("profiles", &self.base);
        for (path, file) in &self.files {
            builder.add_file_fingerprint(path, &file.fingerprint);
        }
        builder.finish()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn scan_module(
        &mut self,
        module_name: &str,
        module_root: &Path,
    ) -> Result<(), ProfileStoreError> {
        let profile_root = ModulePathLayout::new(module_root).profile_root();
        if !profile_root.is_dir() {
            return Ok(());
        }

        for entry in walkdir::WalkDir::new(&profile_root)
            .follow_links(false)
            .into_iter()
        {
            let entry = entry.map_err(|error| ProfileStoreError::Io {
                path: error
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| profile_root.clone()),
                source: std::io::Error::other(error.to_string()),
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().to_path_buf();
            let extension = path.extension().and_then(|value| value.to_str());
            if !matches!(extension, Some("json" | "json5")) {
                continue;
            }

            let kind = ProfileFileKind::from_path(&path);
            let profile = ProfileFile::from_path(&path, module_name, kind.clone())?;

            match kind {
                ProfileFileKind::RouterMap => {
                    self.router_maps
                        .entry(module_name.to_string())
                        .or_default()
                        .files
                        .push(profile.clone());
                }
                ProfileFileKind::StartupConfig => {
                    self.startups
                        .entry(module_name.to_string())
                        .or_default()
                        .files
                        .push(profile.clone());
                }
                ProfileFileKind::ShareConfig => {
                    self.shares
                        .entry(module_name.to_string())
                        .or_default()
                        .files
                        .push(profile.clone());
                }
                ProfileFileKind::Privacy
                | ProfileFileKind::ModuleJson
                | ProfileFileKind::Other { .. } => {}
            }

            self.files.insert(path, profile);
        }
        Ok(())
    }
}

// Static empty inventories for default returns.
static EMPTY_ROUTER_MAP: RouterMapInventory = RouterMapInventory { files: Vec::new() };
static EMPTY_STARTUP: StartupInventory = StartupInventory { files: Vec::new() };
static EMPTY_SHARE: ShareInventory = ShareInventory { files: Vec::new() };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_ref_construction() {
        let ref_ = ProfileRef {
            path: PathBuf::from("/module/profile/router_map.json"),
            module_name: "entry".to_string(),
            kind: ProfileFileKind::RouterMap,
        };
        assert_eq!(ref_.module_name, "entry");
    }

    #[test]
    fn profile_store_empty_for_nonexistent_module() {
        let store = ProfileStore::discover("entry", Path::new("/nonexistent/path"))
            .expect("nonexistent profile root is an empty inventory");
        assert!(store.router_maps_for_module("entry").is_empty());
    }

    #[test]
    fn profile_store_fingerprint_is_deterministic() {
        let store = ProfileStore::default();
        let fp1 = store.fingerprint();
        let fp2 = store.fingerprint();
        assert_eq!(fp1.0, fp2.0);
    }

    #[test]
    fn router_map_inventory_default_is_empty() {
        let inv = RouterMapInventory::default();
        assert!(inv.is_empty());
        assert_eq!(inv.len(), 0);
    }
}
