//! Package acquisition orchestration for the session.
//!
//! Builds `PackageAcquisitionTask`s, calls `arkdown-ohpm`, stores acquisition
//! sources, cache metadata, and diagnostics.

/// OHPM acquisition mode (mirrors `arkdown-ohpm::OhpmResolutionMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionMode {
    /// Only use local files and OHPM cache. Never perform HTTP requests.
    Offline,
    /// Prefer cached packages; fall back to registry.
    PreferOffline,
    /// Always resolve from registry.
    Online,
}

/// A planned package acquisition task.
#[derive(Debug, Clone)]
pub struct PackageAcquisitionTask {
    /// Package name to acquire.
    pub package_name: String,
    /// Requested version constraint or explicit version.
    pub version_spec: Option<String>,
    /// Directory of the declaring package.
    pub issuer_root: std::path::PathBuf,
    /// Whether this is a dev-only dependency.
    pub is_dev: bool,
    /// Whether this is a dynamic dependency.
    pub is_dynamic: bool,
}
