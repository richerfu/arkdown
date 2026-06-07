//! Build session ownership for the arkdown bundler.
//!
//! This crate owns startup discovery and all stable build inputs. After
//! [`BuildSessionBuilder`] completes, later stages consume typed snapshots
//! instead of re-reading config files, package manifests, SDK metadata,
//! profile JSON, and resource trees.
//!
//! # Architecture
//!
//! ```text
//! BuildSessionBuilder
//!   → ProjectSnapshot         (project root, app.json5, modules)
//!   → ModuleSnapshot[]        (module.json5, oh-package.json5, source roots)
//!   → DependencyGraphSnapshot (one per host module, classifies HAR/HSP/SO)
//!   → SdkSnapshot             (SDK paths, API version, component metadata)
//!   → ProfileStore            (indexed profile file store)
//!   → InputFingerprintSet     (content hashes for cache invalidation)
//!   → ArtifactLayout          (output paths)
//!   → BuildSession            (all of the above, Arc-shared)
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! let session = BuildSessionBuilder::new(project_root, config)
//!     .build()
//!     .await?;
//!
//! let ctx: ModuleCompileContext = session.module_context("entry")?;
//! ```

pub mod artifacts;
pub mod builder;
pub mod dependency_graph;
pub mod diagnostics;
pub mod fingerprint;
pub mod module;
pub mod package;
pub mod paths;
pub mod profiles;
pub mod project;
pub mod resources;
pub mod sdk;
pub mod session;

// Public re-exports for consumers.
pub use artifacts::layout::ArtifactLayout;
pub use builder::BuildSessionBuilder;
pub use dependency_graph::snapshot::DependencyGraphSnapshot;
pub use diagnostics::SessionDiagnostics;
pub use fingerprint::InputFingerprintSet;
pub use module::snapshot::ModuleSnapshot;
pub use package::snapshot::PackageSnapshot;
pub use paths::PathResolver;
pub use profiles::store::ProfileStore;
pub use project::snapshot::ProjectSnapshot;
pub use sdk::snapshot::SdkSnapshot;
pub use session::{BuildSession, ModuleCompileContext};
