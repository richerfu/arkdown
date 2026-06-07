use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::ModuleIdx;
use crate::module::SourceMap;

/// Opaque index into [`ChunkGraph::chunks`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkIdx(u32);

impl oxc_index::Idx for ChunkIdx {
    const MAX: usize = u32::MAX as usize;

    fn from_usize(n: usize) -> Self {
        ChunkIdx(n as u32)
    }

    unsafe fn from_usize_unchecked(n: usize) -> Self {
        ChunkIdx(n as u32)
    }

    fn index(self) -> usize {
        self.0 as usize
    }
}

/// A single cross-chunk import item: one export from another chunk that this
/// chunk references, along with the local name it is bound to.
#[derive(Debug, Clone)]
pub struct CrossChunkImportItem {
    /// The name as exported by the other chunk.
    pub export_ref: String,
    /// The name this chunk uses locally for that export.
    pub local_name: String,
}

/// An output chunk produced by the code-splitting stage.
#[derive(Debug, Clone, Default)]
pub struct ArkChunk {
    /// Chunk file name (e.g., `"EntryAbility"`, `"commons"`, `"vendors"`).
    pub name: String,
    /// Content hash for the `globalThis` registry key.
    pub chunk_hash: String,
    /// Modules included in this chunk.
    pub modules: Vec<ModuleIdx>,
    /// Rolldown/Rollup module IDs included in this chunk.
    ///
    /// hvigor's CompileArkTS module-mode backend compiles the full Rollup
    /// module graph (`getModuleIds()`), not just entry chunks. Preserve the
    /// original string IDs so the CLI can generate `filesInfo.txt` from the
    /// same graph surface.
    pub module_ids: Vec<String>,
    /// Whether this chunk is an entry chunk.
    pub is_entry: bool,
    /// Whether this chunk is a dynamic-import entry.
    pub is_dynamic_entry: bool,
    /// The facade module that this chunk represents, if any.
    pub facade_module_id: Option<String>,
    /// Entry module index (for entry chunks).
    pub entry_module: Option<ModuleIdx>,
    /// Final JavaScript source after render hooks.
    pub code: String,
    /// Source map for this chunk.
    pub source_map: Option<SourceMap>,

    // ── Cross-chunk reference tracking ──────────────────────────────────────
    /// Chunk indices that this chunk statically imports from.
    pub cross_chunk_imports: Vec<ChunkIdx>,
    /// Chunk indices that this chunk dynamically imports.
    pub cross_chunk_dynamic_imports: Vec<ChunkIdx>,
    /// Per-source-chunk list of items this chunk imports from it.
    pub imports_from_other_chunks: FxHashMap<ChunkIdx, Vec<CrossChunkImportItem>>,
    /// Map from exported local name → the name exposed to other chunks.
    pub exports_to_other_chunks: FxHashMap<String, String>,

    // ── Bundle metadata (mirrors RenderedChunkMeta) ──────────────────────────
    /// Names exported by this chunk.
    pub exports: Vec<String>,
    /// Specifiers of chunks this chunk imports (for manifest/loader.json).
    pub imports: Vec<String>,
    /// Specifiers of chunks this chunk dynamically imports.
    pub dynamic_imports: Vec<String>,
}

/// The full chunk assignment for one OHOS module build.
#[derive(Debug, Clone, Default)]
pub struct ChunkGraph {
    pub chunks: IndexVec<ChunkIdx, ArkChunk>,
    /// Full Rollup/Rolldown module graph IDs, matching Rollup `getModuleIds()`.
    ///
    /// `ArkChunk::module_ids` only contains modules that survived chunk
    /// generation. hvigor's CompileArkTS cache is written from the full module
    /// graph, so pure re-export/type/const modules that are tree-shaken from
    /// output chunks still need to be preserved here.
    pub full_module_ids: Vec<String>,
    /// Rolldown module-format classification for each full module ID.
    ///
    /// hvigor writes `filesInfo.txt` from Rollup `getModuleIds()` and uses the
    /// module metadata produced by CommonJS analysis instead of rescanning
    /// source files.
    pub full_module_is_commonjs: FxHashMap<String, bool>,
    /// Maps [`ModuleIdx`] → [`ChunkIdx`].
    pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
    /// Maps entry [`ModuleIdx`] → entry [`ChunkIdx`].
    pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
}

impl ChunkGraph {
    /// Return the deduplicated list of all module IDs in the graph.
    ///
    /// Prefers [`ChunkGraph::full_module_ids`] (the complete Rolldown module
    /// graph) when populated; falls back to collecting IDs from every chunk.
    /// Order is stable: IDs appear in the order they are first encountered.
    ///
    /// hvigor's `CompileArkTS` stage compiles the full module graph
    /// (`getModuleIds()`), not just the entry chunks, so this method must be
    /// used wherever the ABC compiler cache is written.
    pub fn all_module_ids(&self) -> Vec<String> {
        let mut seen = FxHashSet::default();
        let mut ids = Vec::new();

        if !self.full_module_ids.is_empty() {
            for id in &self.full_module_ids {
                if seen.insert(id.clone()) {
                    ids.push(id.clone());
                }
            }
            return ids;
        }

        for chunk in &self.chunks {
            for id in &chunk.module_ids {
                if seen.insert(id.clone()) {
                    ids.push(id.clone());
                }
            }
        }
        ids
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ModuleIdx;
    use oxc_index::Idx as _;

    // ── ChunkIdx ─────────────────────────────────────────────────────────────

    #[test]
    fn test_chunk_idx_from_usize_round_trip() {
        let idx = ChunkIdx::from_usize(7);
        assert_eq!(idx.index(), 7);
    }

    #[test]
    fn test_chunk_idx_ordering() {
        let a = ChunkIdx::from_usize(0);
        let b = ChunkIdx::from_usize(1);
        assert!(a < b);
    }

    // ── ArkChunk default ─────────────────────────────────────────────────────

    #[test]
    fn test_ark_chunk_default_is_empty_and_zero() {
        let chunk = ArkChunk::default();
        assert!(chunk.name.is_empty());
        assert!(chunk.chunk_hash.is_empty());
        assert!(chunk.modules.is_empty());
        assert!(!chunk.is_entry);
        assert!(!chunk.is_dynamic_entry);
        assert!(chunk.facade_module_id.is_none());
        assert!(chunk.entry_module.is_none());
        assert!(chunk.code.is_empty());
        assert!(chunk.source_map.is_none());
        assert!(chunk.cross_chunk_imports.is_empty());
        assert!(chunk.cross_chunk_dynamic_imports.is_empty());
        assert!(chunk.imports_from_other_chunks.is_empty());
        assert!(chunk.exports_to_other_chunks.is_empty());
        assert!(chunk.exports.is_empty());
        assert!(chunk.imports.is_empty());
        assert!(chunk.dynamic_imports.is_empty());
    }

    #[test]
    fn test_ark_chunk_no_panic_on_default() {
        // Just constructing Default must not panic.
        let _chunk = ArkChunk::default();
    }

    // ── ArkChunk with cross_chunk_imports ────────────────────────────────────

    #[test]
    fn test_ark_chunk_cross_chunk_imports_accessible() {
        let mut chunk = ArkChunk::default();
        chunk.cross_chunk_imports.push(ChunkIdx::from_usize(0));
        chunk.cross_chunk_imports.push(ChunkIdx::from_usize(2));
        assert_eq!(chunk.cross_chunk_imports.len(), 2);
        assert_eq!(chunk.cross_chunk_imports[0], ChunkIdx::from_usize(0));
        assert_eq!(chunk.cross_chunk_imports[1], ChunkIdx::from_usize(2));
    }

    #[test]
    fn test_ark_chunk_entry_fields() {
        let idx = ModuleIdx::from_usize(3);
        let mut chunk = ArkChunk::default();
        chunk.name = "EntryAbility".to_string();
        chunk.chunk_hash = "deadbeef".to_string();
        chunk.is_entry = true;
        chunk.entry_module = Some(idx);
        assert_eq!(chunk.name, "EntryAbility");
        assert_eq!(chunk.chunk_hash, "deadbeef");
        assert!(chunk.is_entry);
        assert_eq!(chunk.entry_module, Some(idx));
    }

    #[test]
    fn test_ark_chunk_dynamic_entry() {
        let mut chunk = ArkChunk::default();
        chunk.is_dynamic_entry = true;
        chunk.facade_module_id = Some("lazy-page".to_string());
        assert!(chunk.is_dynamic_entry);
        assert_eq!(chunk.facade_module_id.as_deref(), Some("lazy-page"));
    }

    // ── CrossChunkImportItem ─────────────────────────────────────────────────

    #[test]
    fn test_cross_chunk_import_item_construction() {
        let item = CrossChunkImportItem {
            export_ref: "EntryAbilityClass".to_string(),
            local_name: "EntryAbilityClass".to_string(),
        };
        assert_eq!(item.export_ref, "EntryAbilityClass");
        assert_eq!(item.local_name, "EntryAbilityClass");
    }

    #[test]
    fn test_cross_chunk_import_item_renamed() {
        let item = CrossChunkImportItem {
            export_ref: "OriginalName".to_string(),
            local_name: "_localAlias".to_string(),
        };
        assert_ne!(item.export_ref, item.local_name);
    }

    // ── ChunkGraph ───────────────────────────────────────────────────────────

    #[test]
    fn test_chunk_graph_empty_construction() {
        let graph = ChunkGraph {
            chunks: IndexVec::new(),
            full_module_ids: Vec::new(),
            full_module_is_commonjs: FxHashMap::default(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        };
        assert!(graph.chunks.is_empty());
        assert!(graph.module_to_chunk.is_empty());
        assert!(graph.entry_module_to_entry_chunk.is_empty());
    }

    #[test]
    fn test_chunk_graph_with_one_chunk() {
        let mut chunk = ArkChunk::default();
        chunk.name = "commons".to_string();
        chunk.is_entry = false;

        let mut chunks: IndexVec<ChunkIdx, ArkChunk> = IndexVec::new();
        chunks.push(chunk);

        let graph = ChunkGraph {
            chunks,
            full_module_ids: Vec::new(),
            full_module_is_commonjs: FxHashMap::default(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        };
        assert_eq!(graph.chunks.len(), 1);
        assert_eq!(graph.chunks[ChunkIdx::from_usize(0)].name, "commons");
    }
}
