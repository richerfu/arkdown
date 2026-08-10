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
//!   → SourceStore             (source file inventory and fingerprints)
//!   → ResolverIndexSnapshot   (resolver configuration per module)
//!   → BuildScriptSnapshot[]   (hvigorfile.ts / hvigorconfig.ts declarative effects)
//!   → BuildSession            (all of the above, Arc-shared)
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! let session = BuildSessionBuilder::new(config)
//!     .build()
//!     .await?;
//!
//! let ctx: ModuleCompileContext = session.module_context("entry")?;
//! ```

pub mod artifacts;
pub mod build_script;
pub mod builder;
pub mod context;
pub mod dependency_graph;
pub mod dependency_views;
pub mod diagnostics;
pub mod duplicate_dependency;
pub mod fingerprint;
pub mod hvigor_context;
pub mod mode_inventory;
pub mod module;
pub mod native_inventory;
pub mod package;
pub mod paths;
pub mod plan_input;
pub mod profiles;
pub mod project;
pub mod resolver_index;
pub mod resource_inventory;
pub mod resources;
pub mod sdk;
pub mod sdk_compile_plugin;
pub mod sdk_schemas;
pub mod sdk_tools;
pub mod session;
pub mod source_store;

// ---------------------------------------------------------------------------
// Narrow re-exports (SPEC-048)
// ---------------------------------------------------------------------------

// artifacts
pub use artifacts::layout::ArtifactLayout;

// build_script
pub use build_script::{
    BuildScriptEffect, BuildScriptFileInventory, BuildScriptFileInventoryRequest,
    BuildScriptFingerprint, BuildScriptKind, BuildScriptModuleModelLoader,
    BuildScriptModuleModelRequest, BuildScriptOverrideError, BuildScriptOverrideInventory,
    BuildScriptParser, BuildScriptProjectCacheIdentity, BuildScriptProjectCacheIdentityError,
    BuildScriptRuntimeModelError, BuildScriptRuntimeModelLoader, BuildScriptRuntimeModelRequest,
    BuildScriptRuntimeModelSnapshot, BuildScriptSnapshot, ParseBuildScriptError,
};

// builder
pub use builder::BuildSessionBuilder;

// context — narrow view types
pub use context::{ArtifactView, DependencyView, FingerprintView};

// dependency_graph
pub use dependency_graph::snapshot::DependencyGraphSnapshot;

// diagnostics
pub use diagnostics::SessionDiagnostics;

// duplicate_dependency
pub use duplicate_dependency::{
    DuplicateDependencyError, DuplicateDependencyValidator, DuplicateHarDependency,
};

// fingerprint
pub use fingerprint::InputFingerprintSet;

// module
pub use module::snapshot::ModuleSnapshot;

// package
pub use package::snapshot::PackageSnapshot;

// paths
pub use paths::PathResolver;

// plan_input — narrow stage input views for CLI BuildPlanBuilder
pub use plan_input::{
    BundlePlanInput, BytecodePlanInput, NativePlanInput, PackagePlanInput, ProfilePlanInput,
    ResourcePlanInput, SignPlanInput, WatchPlanInput,
};

// sdk
pub use sdk::SdkEs2abcOptionSnapshot;

// profiles
pub use profiles::store::{
    ParsedProfile, ProfileFile, ProfileFileKind, ProfileRef, ProfileStore, RouterMapInventory,
    ShareInventory, StartupInventory,
};

// project
pub use project::snapshot::ProjectSnapshot;

// resolver_index
pub use resolver_index::{
    ExtensionSet, PackageIndexEntry, ResolverIndexBuilder, ResolverIndexSnapshot, SdkModuleEntry,
    SourceRootEntry,
};

// sdk
pub use sdk::snapshot::SdkSnapshot;

// session
pub use session::{BuildSession, ModuleCompileContext};

// source_store
pub use source_store::{
    CodeResourceInventory, SourceContent, SourceFile, SourceFingerprint, SourceStore,
    VisualSourceInventory, WorkerSourceInventory,
};

// dependency_views
pub use dependency_views::{
    ByteCodeHarDependencyView, DynamicDependencyView, HarDependencyView, HspDependencyView,
    NativeSoDependencyView, RemoteHspDependencyView,
};
pub use hvigor_context::{
    HvigorContextInspectError, HvigorContextInspector, HvigorContextSnapshot,
    HvigorDependencyContextSnapshot, OhpmDependencyInfo,
};

// sdk_tools
pub use sdk_tools::{SdkToolKind, SdkToolPath, SdkToolResolver};

// sdk_schemas
pub use sdk_schemas::{SdkSchemaKind, SdkSchemaPath, SdkSchemaResolver};

// sdk_compile_plugin
pub use sdk_compile_plugin::{
    ComponentMetadata, EtsLoaderPath, JsLoaderPath, SdkArkTsFeatureGate, SdkCompilePluginSnapshot,
    SdkSupportExtensionSet,
};

// native_inventory
pub use native_inventory::{
    NativeAbi, NativeBuildInventory, NativeBuildTarget, NativeDebugSymbolPolicy, NativeLibCandidate,
};

// mode_inventory
pub use mode_inventory::{
    BuildModeInventory, CoverageInventory, OhosTestInventory, PreviewInventory, ReloadInventory,
    TestInventory, TestRunnerKind,
};

// resource_inventory
pub use resource_inventory::{NativeLibInventory, ResourceInput, ResourceInventory, ResourceRoot};
