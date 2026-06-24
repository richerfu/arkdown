//! Typed build-context identifiers.
//!
//! Each ID type is an opaque newtype over `u32` that implements [`oxc_index::Idx`]
//! so it can be used as an index into [`oxc_index::IndexVec`].
//!
//! These IDs represent entities in the **session / build plan** context:
//! - [`ProjectId`] — the root project
//! - [`ModuleId`] — an OHOS module (`"entry"`, `"feature"`, a HAR, etc.)
//! - [`TargetId`] — a compilation target within a module
//! - [`PackageId`] — an `oh-package` dependency node (local, HAR, HSP)
//! - [`DependencyNodeId`] — a node in the dependency graph
//! - [`SourceFileId`] — a source file (`*.ets`/`*.ts`/`*.js`) tracked by
//!   the source store
//! - [`ResourceFileId`] — a resource file tracked by the resource inventory
//! - [`ProfileFileId`] — a profile JSON5 file tracked by the profile store
//! - [`CompileUnitId`] — a per-file ABC compile unit inside a typed plan
//! - [`ChunkId`] — a bundler output chunk
//! - [`PackEntryId`] — a HAP/HSP/HAR package entry artifact
//!
//! The bundler's internal `ModuleIdx` and `ChunkIdx` (in `graph.rs` and
//! `chunk.rs`) are separate — they index into the Rolldown module/chunk graph
//! and are NOT the same as `ModuleId`/`ChunkId` here.

macro_rules! define_id {
    ($(#[$attr:meta])* $vis:vis struct $name:ident;) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $vis struct $name(pub u32);

        impl $name {
            /// Create a new ID from a `usize`.
            #[inline]
            pub fn new(n: usize) -> Self {
                Self(n as u32)
            }

            /// Return the underlying integer index.
            #[inline]
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl oxc_index::Idx for $name {
            const MAX: usize = u32::MAX as usize;

            #[inline]
            fn from_usize(n: usize) -> Self {
                Self(n as u32)
            }

            #[inline]
            unsafe fn from_usize_unchecked(n: usize) -> Self {
                Self(n as u32)
            }

            #[inline]
            fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(
    /// Identity of the root project.
    ///
    /// Corresponds to the hvigor project model's `projectName`.
    pub struct ProjectId;
);

define_id!(
    /// Identity of an OHOS module (`"entry"`, `"feature"`, a HAR, an HSP, etc.).
    ///
    /// Corresponds to `ModuleSnapshot` in `arkdown-session`.
    /// This is the **session-context module ID**, NOT the bundler's `ModuleIdx`.
    pub struct ModuleId;
);

define_id!(
    /// Identity of a compilation target within a module.
    ///
    /// A module may define multiple targets (e.g., `default`, `phone`, `tv`).
    /// Corresponds to `ModuleTargetSnapshot` in `arkdown-session`.
    pub struct TargetId;
);

define_id!(
    /// Identity of an `oh-package` dependency node in the resolved dependency
    /// graph.
    ///
    /// Corresponds to `PackageSnapshot` in `arkdown-session`.
    pub struct PackageId;
);

define_id!(
    /// Identity of a node in the dependency graph.
    ///
    /// Corresponds to `DependencyNode` in `arkdown-session::dependency_graph`.
    pub struct DependencyNodeId;
);

define_id!(
    /// Identity of a source file tracked by `SourceStore`.
    ///
    /// Used to reference `*.ets`/`*.ts`/`*.js` files in the compile-entry set.
    /// Corresponds to `SourceFile` in `arkdown-session::source_store`.
    pub struct SourceFileId;
);

define_id!(
    /// Identity of a resource file tracked by `ResourceInventory`.
    ///
    /// Used to reference `*.json`/`*.jpg`/etc. in the resource compile plan.
    pub struct ResourceFileId;
);

define_id!(
    /// Identity of a profile JSON5 file tracked by `ProfileStore`.
    ///
    /// Used to reference router-map, startup, share-config, etc. profile files.
    pub struct ProfileFileId;
);

define_id!(
    /// Identity of an ABC compile unit within a typed `Ts2abcPlan` or
    /// `AotCompilePlan`.
    ///
    /// Corresponds to `CompileUnitPlan` in `arkdown-ts2abc::plan`.
    pub struct CompileUnitId;
);

define_id!(
    /// Identity of a bundler output chunk.
    ///
    /// Used in chunk graphs and manual-chunk policies.
    /// This is the **typed-plan chunk ID**, NOT the bundler's `ChunkIdx`.
    pub struct ChunkId;
);

define_id!(
    /// Identity of a HAP/HSP/HAR package entry artifact.
    ///
    /// Corresponds to a single file entry in a `PackagePlan`.
    pub struct PackEntryId;
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_round_trips_through_usize() {
        for n in [0usize, 1, 100, u32::MAX as usize] {
            let id = ModuleId::new(n);
            assert_eq!(id.index(), n);
        }
    }

    #[test]
    fn chunk_id_implements_idx_trait() {
        let id: ChunkId = oxc_index::Idx::from_usize(7);
        assert_eq!(oxc_index::Idx::index(id), 7);
    }

    #[test]
    fn all_id_types_are_distinct() {
        // Ensure type safety — identical raw values don't compare equal across types.
        let m = ModuleId::new(42);
        let p = PackageId::new(42);
        assert_eq!(m.index(), p.index()); // same raw value...
                                          // ...but they are distinct types (won't compile if assigned interchangeably).
        let _ = m;
        let _ = p;
    }
}
