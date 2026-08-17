//! Output contract of a successful [`crate::Bundler::bundle`] run.

use std::path::PathBuf;

use rustc_hash::FxHashMap;

use arkdown_core::{ChunkGraph, ChunkSourceFile, HvigorModuleSourceFile, HvigorNpmEntry};

use crate::bundler::*;

/// The output produced by a successful [`Bundler::bundle`] call.
pub struct BundleOutput {
    pub chunk_graph: ChunkGraph,
    pub output_dir: PathBuf,
    /// `(chunk_name, absolute_path)` for each written `.js` file.
    pub chunk_files: Vec<(String, PathBuf)>,
    /// `(chunk_name, source path relative to src/main)` for compile-entry chunks.
    ///
    /// The bytecode stage uses this to derive hvigor-compatible ABC record names
    /// for `filesInfo.txt`.
    pub chunk_source_files: Vec<ChunkSourceFile>,
    /// Transformed module sources compiled by SDK es2abc in hvigor ESMODULE mode.
    pub module_source_files: Vec<HvigorModuleSourceFile>,
    /// Entry module IDs passed to Rolldown scan.
    pub entry_module_ids: Vec<String>,
    /// Module graph import edges collected from Rolldown/OXC module records.
    pub module_imports: FxHashMap<String, Vec<String>>,
    /// External module requests collected from Rolldown module import records.
    ///
    /// The bytecode stage lowers these to hvigor-compatible `npmEntries.txt`
    /// entries before invoking SDK es2abc.
    pub external_module_requests: Vec<String>,
    /// Importee inventories written by hvigor's ArkTS `processImportees`
    /// build step and consumed by ByteCode HAR artifact processing.
    pub har_importees: HvigorHarImportees,
    /// System/native imports reachable from hvigor preload entry roots.
    /// Written by the CLI as `intermediates/preload/<target>/preload.json`.
    pub preload_system_so_tasks: Vec<PreloadSystemSoTask>,
    /// Package and local dependency mappings written to `npmEntries.txt`.
    pub npm_entries: Vec<HvigorNpmEntry>,
    /// Native ABC backend output path. Packagers must use this path as the
    /// source for the final archive entry `ets/modules.abc`.
    pub modules_abc_path: PathBuf,
    /// `sourceMaps.map` if a real source map was produced from bundler inputs.
    pub source_maps_path: Option<PathBuf>,
    /// Content fingerprints for source-map entries whose original source was
    /// already inventoried by the build session. The CLI cache handoff uses
    /// these to retain latest ets-loader's per-module incremental map order
    /// without re-reading source files after Rolldown finishes.
    pub source_map_input_fingerprints: Vec<SourceMapInputFingerprint>,
}
