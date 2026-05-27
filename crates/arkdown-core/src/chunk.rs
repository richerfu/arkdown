use std::ops::Range;

use index_vec::IndexVec;
use rustc_hash::FxHashMap;

use crate::graph::ModuleIdx;
use crate::module::SourceMap;

/// Opaque index into [`ChunkGraph::chunks`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkIdx(u32);

impl index_vec::Idx for ChunkIdx {
    fn from_usize(n: usize) -> Self {
        ChunkIdx(n as u32)
    }
    fn index(self) -> usize {
        self.0 as usize
    }
}

/// An output chunk produced by the code-splitting stage.
#[derive(Debug, Clone)]
pub struct ArkChunk {
    /// Chunk file name (e.g., `"EntryAbility"`, `"commons"`, `"vendors"`).
    pub name: String,
    /// Content hash for the `globalThis` registry key.
    pub chunk_hash: String,
    /// Modules included in this chunk.
    pub modules: Vec<ModuleIdx>,
    /// Whether this chunk is an entry chunk.
    pub is_entry: bool,
    /// Entry module index (for entry chunks).
    pub entry_module: Option<ModuleIdx>,
    /// IIFE-wrapped JavaScript source (after `renderChunk`).
    pub code: String,
    /// Source map for this chunk.
    pub source_map: Option<SourceMap>,
}

/// The full chunk assignment for one OHOS module build.
#[derive(Debug, Clone)]
pub struct ChunkGraph {
    pub chunks: IndexVec<ChunkIdx, ArkChunk>,
    /// Maps [`ModuleIdx`] → [`ChunkIdx`].
    pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
    /// Maps entry [`ModuleIdx`] → entry [`ChunkIdx`].
    pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
}

/// The final IIFE-wrapped chunk as produced by `arkdown-plugin-chunk-wrapper`.
/// This is the direct input to `arkdown-abc`.
#[derive(Debug, Clone)]
pub struct IifeChunk {
    pub chunk: ArkChunk,
    /// Cross-chunk import replacements made by the chunk-wrapper plugin.
    /// Maps `"chunkId"` → `"globalThis['chunkId_hash']"`.
    pub require_replacements: Vec<RequireReplacement>,
    /// Global registry export appended at end of chunk:
    /// `"globalThis['fileName_hash'] = { export1: exports.export1, ... }"`.
    pub registry_export: Option<String>,
}

/// One `require(...)` → `globalThis[...]` replacement performed by the chunk-wrapper plugin.
#[derive(Debug, Clone)]
pub struct RequireReplacement {
    /// Original call, e.g. `require('chunkId')`.
    pub original: String,
    /// Replacement expression, e.g. `globalThis['chunkId_hash']`.
    pub replacement: String,
    /// Byte range of the original text in the chunk source.
    pub byte_range: Range<usize>,
}
