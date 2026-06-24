//! Resolver index for module resolution.
//!
//! [`ResolverIndexSnapshot`] is built once per module from the session's
//! dependency graph, package index, and SDK snapshot.  It is consumed by
//! `arkdown-resolver::OhosResolver` to resolve module specifiers without
//! performing any filesystem discovery at resolution time.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `ResolverIndexBuilder` | `arkts-pack.js::getResolveModules` |
//! | `ResolverIndexSnapshot` | rollup `resolve.modules` configuration |

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// PackageIndexEntry
// ---------------------------------------------------------------------------

/// An entry in the package resolution index.
///
/// Maps an ohpm package name to its root directory and entry file.
///
/// # Upstream correspondence
///
/// Corresponds to one element of `rollup.resolve.modules` or
/// `getResolveModules` in `arkts-pack.js`.
#[derive(Debug, Clone)]
pub struct PackageIndexEntry {
    /// The package name (e.g., `"@ohos/utils"`, `"my-har"`).
    pub name: String,
    /// The package root directory (`oh_modules/<name>` or local source root).
    pub root: PathBuf,
    /// The package's main entry file path.
    pub entry: Option<PathBuf>,
    /// The package version (from `oh-package.json5`).
    pub version: Option<String>,
}

// ---------------------------------------------------------------------------
// SourceRootEntry
// ---------------------------------------------------------------------------

/// A source root directory entry for the resolver.
///
/// The resolver searches these directories for relative-specifier resolution.
///
/// # Upstream correspondence
///
/// Corresponds to the `resolve.roots` configuration in rollup/rolldown.
#[derive(Debug, Clone)]
pub struct SourceRootEntry {
    /// Absolute path to the source root directory.
    pub path: PathBuf,
    /// Whether this is the module's primary source root.
    pub is_primary: bool,
}

// ---------------------------------------------------------------------------
// ExtensionSet
// ---------------------------------------------------------------------------

/// The set of file extensions the resolver should try for extensionless
/// imports.
///
/// # Upstream correspondence
///
/// Corresponds to `resolve.extensions` in rollup configuration and
/// `getSupportExtensions` from the SDK snapshot.
#[derive(Debug, Clone)]
pub struct ExtensionSet {
    /// Extensions in priority order (e.g., `[".ets", ".ts", ".js"]`).
    pub extensions: Vec<String>,
}

impl Default for ExtensionSet {
    fn default() -> Self {
        Self {
            extensions: vec![
                ".ets".to_string(),
                ".ts".to_string(),
                ".js".to_string(),
                ".json".to_string(),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// SdkModuleEntry
// ---------------------------------------------------------------------------

/// An SDK API module entry in the resolver index.
///
/// SDK modules (e.g., `@ohos/hilog`, `@system/request`) are resolved from
/// the SDK's `ets/api/` and `kits/` directories.
///
/// # Upstream correspondence
///
/// Corresponds to SDK API modules in `DefaultSdk.getSdkEtsDir/getSdkJsDir`
/// and kit imports from `DefaultSdk.getKitApiPath`.
#[derive(Debug, Clone)]
pub struct SdkModuleEntry {
    /// The SDK module specifier (e.g., `"@ohos/hilog"`).
    pub specifier: String,
    /// Absolute path to the SDK API file.
    pub api_path: PathBuf,
    /// Whether this is a kit import (uses kit alias resolution).
    pub is_kit: bool,
}

// ---------------------------------------------------------------------------
// ResolverIndexSnapshot
// ---------------------------------------------------------------------------

/// A snapshot of the resolver configuration for one OHOS module.
///
/// `ResolverIndexSnapshot` is built once by `ResolverIndexBuilder` and shared
/// (via `Arc`) with the rolldown plugin and all resolution calls.  It does not
/// perform any filesystem reads — all paths are pre-resolved.
///
/// # Upstream correspondence
///
/// Corresponds to the `rollup.resolve` configuration assembled by
/// `arkts-pack.js::getResolveModules`.
#[derive(Debug, Clone, Default)]
pub struct ResolverIndexSnapshot {
    /// ohpm package index: name → entry.
    pub packages: Vec<PackageIndexEntry>,
    /// Source root directories.
    pub source_roots: Vec<SourceRootEntry>,
    /// SDK API modules.
    pub sdk_modules: Vec<SdkModuleEntry>,
    /// File extension resolution order.
    pub extensions: ExtensionSet,
}

impl ResolverIndexSnapshot {
    /// Look up a package entry by name.
    pub fn package_by_name(&self, name: &str) -> Option<&PackageIndexEntry> {
        self.packages.iter().find(|p| p.name == name)
    }

    /// Find the package index entry for a file path.
    ///
    /// Returns the entry whose `root` is the longest prefix of `path`.
    pub fn package_for_path(&self, path: &std::path::Path) -> Option<&PackageIndexEntry> {
        self.packages
            .iter()
            .filter(|p| path.starts_with(&p.root))
            .max_by_key(|p| p.root.components().count())
    }

    /// Return all source roots.
    pub fn source_roots(&self) -> &[SourceRootEntry] {
        &self.source_roots
    }

    /// Return all SDK module entries.
    pub fn sdk_modules(&self) -> &[SdkModuleEntry] {
        &self.sdk_modules
    }
}

// ---------------------------------------------------------------------------
// ResolverIndexBuilder
// ---------------------------------------------------------------------------

/// Builds a [`ResolverIndexSnapshot`] from session data.
///
/// `ResolverIndexBuilder` is called once per module by `BuildSessionBuilder`
/// (or lazily by the CLI planner) to produce the resolver configuration for
/// rolldown plugin construction.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getResolveModules` and the rolldown
/// `resolve.modules` configuration assembly.
pub struct ResolverIndexBuilder {
    packages: Vec<PackageIndexEntry>,
    source_roots: Vec<SourceRootEntry>,
    sdk_modules: Vec<SdkModuleEntry>,
    extensions: ExtensionSet,
}

impl ResolverIndexBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self {
            packages: Vec::new(),
            source_roots: Vec::new(),
            sdk_modules: Vec::new(),
            extensions: ExtensionSet::default(),
        }
    }

    /// Add a package entry to the index.
    pub fn add_package(&mut self, entry: PackageIndexEntry) -> &mut Self {
        self.packages.push(entry);
        self
    }

    /// Add a source root to the index.
    pub fn add_source_root(&mut self, entry: SourceRootEntry) -> &mut Self {
        self.source_roots.push(entry);
        self
    }

    /// Add an SDK module entry to the index.
    pub fn add_sdk_module(&mut self, entry: SdkModuleEntry) -> &mut Self {
        self.sdk_modules.push(entry);
        self
    }

    /// Set the extension resolution order.
    pub fn with_extensions(&mut self, extensions: ExtensionSet) -> &mut Self {
        self.extensions = extensions;
        self
    }

    /// Convenience: build the index for a module from the session's data.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to assembling `getResolveModules` from the module's
    /// `oh-package.json5` dependencies and SDK module directories.
    pub fn build_for_module(self) -> ResolverIndexSnapshot {
        ResolverIndexSnapshot {
            packages: self.packages,
            source_roots: self.source_roots,
            sdk_modules: self.sdk_modules,
            extensions: self.extensions,
        }
    }

    /// Return the built snapshot's package index.
    pub fn package_index(&self) -> &[PackageIndexEntry] {
        &self.packages
    }

    /// Return the built snapshot's SDK module index.
    pub fn sdk_index(&self) -> &[SdkModuleEntry] {
        &self.sdk_modules
    }

    /// Return the built snapshot's source root index.
    pub fn source_root_index(&self) -> &[SourceRootEntry] {
        &self.source_roots
    }
}

impl Default for ResolverIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_lookup_by_name() {
        let mut builder = ResolverIndexBuilder::new();
        builder.add_package(PackageIndexEntry {
            name: "@ohos/utils".to_string(),
            root: PathBuf::from("/app/oh_modules/@ohos/utils"),
            entry: Some(PathBuf::from("/app/oh_modules/@ohos/utils/index.ets")),
            version: Some("1.0.0".to_string()),
        });
        let snapshot = builder.build_for_module();
        assert!(snapshot.package_by_name("@ohos/utils").is_some());
        assert!(snapshot.package_by_name("nonexistent").is_none());
    }

    #[test]
    fn package_for_path_returns_deepest_match() {
        let mut builder = ResolverIndexBuilder::new();
        builder.add_package(PackageIndexEntry {
            name: "har-a".to_string(),
            root: PathBuf::from("/app/oh_modules/har-a"),
            entry: None,
            version: None,
        });
        let snapshot = builder.build_for_module();
        let path = PathBuf::from("/app/oh_modules/har-a/src/utils.ets");
        assert!(snapshot.package_for_path(&path).is_some());
        assert_eq!(snapshot.package_for_path(&path).unwrap().name, "har-a");
    }
}
