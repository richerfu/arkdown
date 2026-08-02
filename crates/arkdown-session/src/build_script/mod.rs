//! OXC-based declarative build script analysis.
//!
//! This module replaces the old `hvigor_script.rs` and `hvigor_task_graph.rs`
//! modules.  It parses `hvigorfile.ts` and `hvigorconfig.ts` with OXC and
//! extracts the declarative [`effects::BuildScriptEffect`]s that arkdown can
//! represent without executing JavaScript.
//!
//! # Module layout
//!
//! - [`effects`] — `BuildScriptEffect` enum and effect sub-types.
//! - [`snapshot`] — `BuildScriptSnapshot`, `BuildScriptKind`, and
//!   `BuildScriptFingerprint`.
//! - [`parse`] — `BuildScriptParser` backed by OXC.
//!
//! # Upstream correspondence
//!
//! Replaces `HvigorScript*` and `HvigorTaskGraph*` public types from the
//! previous `hvigor_script.rs` / `hvigor_task_graph.rs` modules.  See
//! SPEC-007 §`arkdown-session` for the migration rationale.

pub mod cache_identity;
pub mod effects;
pub mod inventory;
pub mod parse;
pub mod runtime_model;
pub mod snapshot;

// Narrow re-exports for callers that only need the primary types.
pub use cache_identity::{BuildScriptProjectCacheIdentity, BuildScriptProjectCacheIdentityError};
pub use effects::BuildScriptEffect;
pub use inventory::{BuildScriptOverrideError, BuildScriptOverrideInventory};
pub use parse::{BuildScriptParser, ParseBuildScriptError};
pub use runtime_model::{
    BuildScriptFileInventory, BuildScriptFileInventoryRequest, BuildScriptModuleModelLoader,
    BuildScriptModuleModelRequest, BuildScriptRuntimeModelError, BuildScriptRuntimeModelLoader,
    BuildScriptRuntimeModelRequest, BuildScriptRuntimeModelSnapshot,
};
pub use snapshot::{BuildScriptFingerprint, BuildScriptKind, BuildScriptSnapshot};
