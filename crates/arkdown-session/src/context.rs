//! Module compile context view types.
//!
//! This module defines narrow view types used by domain crates to access
//! specific aspects of the per-module compile context:
//!
//! - [`DependencyView`]: narrow view over the dependency graph
//! - [`ArtifactView`]: narrow view over artifact output paths
//! - [`FingerprintView`]: stage-scoped fingerprint view for cache keys
//!
//! The full [`crate::session::ModuleCompileContext`] is the central context
//! object; these view types provide narrower typed access to specific aspects.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `DependencyView` | `TaskService.getDependencyView` |
//! | `ArtifactView` | `PathInfo` output paths |
//! | `FingerprintView` | `invalid-cache-plugin.js` per-stage fingerprint |
//! | `ModuleCompileContext` (session.rs) | `TargetTaskService` context |

use std::sync::Arc;

use crate::artifacts::layout::ArtifactLayout;
use crate::dependency_graph::snapshot::DependencyGraphSnapshot;
use crate::fingerprint::InputFingerprintSet;

// ---------------------------------------------------------------------------
// DependencyView
// ---------------------------------------------------------------------------

/// A narrow view over a module's dependency graph for use by domain crates.
///
/// Domain crates (profile, resource, pack, ts2abc) should consume this view
/// rather than the full `DependencyGraphSnapshot` to limit their surface.
///
/// # Upstream correspondence
///
/// Corresponds to `TaskService.getDependencyView` in hvigor, which provides
/// a read-only dependency graph view to the task implementation.
#[derive(Debug, Clone)]
pub struct DependencyView {
    inner: Arc<DependencyGraphSnapshot>,
}

impl DependencyView {
    /// Create a dependency view from a snapshot.
    pub fn new(snapshot: Arc<DependencyGraphSnapshot>) -> Self {
        Self { inner: snapshot }
    }

    /// Access the underlying snapshot (for session-level code).
    pub fn snapshot(&self) -> &DependencyGraphSnapshot {
        &self.inner
    }

    /// Return the HAR dependency names for this module.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `TaskService.getRemoteHarDependencies`.
    pub fn har_dependency_names(&self) -> Vec<&str> {
        self.inner
            .nodes
            .values()
            .filter(|n| {
                matches!(
                    n.node_type,
                    crate::dependency_graph::snapshot::DependencyNodeType::SourceHar
                        | crate::dependency_graph::snapshot::DependencyNodeType::BytecodeHar
                )
            })
            .map(|n| n.package_name.as_str())
            .collect()
    }

    /// Return the HSP dependency names for this module.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `TaskService.getOhpmRemoteHspDependencies`.
    pub fn hsp_dependency_names(&self) -> Vec<&str> {
        self.inner
            .nodes
            .values()
            .filter(|n| {
                matches!(
                    n.node_type,
                    crate::dependency_graph::snapshot::DependencyNodeType::SharedHsp
                        | crate::dependency_graph::snapshot::DependencyNodeType::IntegratedHsp
                )
            })
            .map(|n| n.package_name.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ArtifactView
// ---------------------------------------------------------------------------

/// A narrow view over the module's artifact output paths.
///
/// Domain crates use this to locate their output files without needing the
/// full `ArtifactLayout` structure.
///
/// # Upstream correspondence
///
/// Corresponds to `PathInfo` and `BuildDirConst` in hvigor.
#[derive(Debug, Clone)]
pub struct ArtifactView {
    inner: Arc<ArtifactLayout>,
}

impl ArtifactView {
    /// Create an artifact view from a layout.
    pub fn new(layout: Arc<ArtifactLayout>) -> Self {
        Self { inner: layout }
    }

    /// Access the underlying layout.
    pub fn layout(&self) -> &ArtifactLayout {
        &self.inner
    }

    /// The output root directory for this module.
    pub fn output_root(&self) -> &std::path::Path {
        &self.inner.output_root
    }

    /// The loader.json path.
    pub fn loader_json_path(&self) -> std::path::PathBuf {
        self.inner.loader_json()
    }

    /// The `ets/modules.abc` path.
    pub fn abc_path(&self) -> std::path::PathBuf {
        self.inner.modules_abc()
    }
}

// ---------------------------------------------------------------------------
// FingerprintView
// ---------------------------------------------------------------------------

/// A view over the session's input fingerprints for a specific build stage.
///
/// Domain crates use this to compute cache keys for their stage outputs.
///
/// # Upstream correspondence
///
/// Corresponds to the per-stage cache key inputs collected by
/// `invalid-cache-plugin.js`.
#[derive(Debug, Clone)]
pub struct FingerprintView {
    inner: Arc<InputFingerprintSet>,
    stage_name: String,
}

impl FingerprintView {
    /// Create a fingerprint view for a named stage.
    pub fn new(fingerprints: Arc<InputFingerprintSet>, stage_name: impl Into<String>) -> Self {
        Self {
            inner: fingerprints,
            stage_name: stage_name.into(),
        }
    }

    /// The stage name for which this fingerprint view was created.
    pub fn stage_name(&self) -> &str {
        &self.stage_name
    }

    /// Access the underlying fingerprint set.
    pub fn fingerprints(&self) -> &InputFingerprintSet {
        &self.inner
    }
}
