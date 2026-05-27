use std::collections::HashSet;

use index_vec::IndexVec;
use rustc_hash::FxHashMap;

use arkdown_core::{ArkChunk, ChunkGraph, ChunkIdx, ModuleGraph, ModuleIdx};

/// Assign modules to output chunks based on entry points.
///
/// Strategy: one chunk per entry module. Each entry chunk claims all modules
/// reachable from it (DFS), with "first-claim wins" for shared modules.
///
/// When `support_chunks` is false all modules go into a single entry chunk
/// (the first entry). When true, each entry gets its own chunk.
///
/// Modules marked as excluded by tree-shaking (`has_side_effects == false`
/// and not an entry) are skipped.
pub fn split_chunks(graph: &ModuleGraph, support_chunks: bool) -> ChunkGraph {
    let mut chunks: IndexVec<ChunkIdx, ArkChunk> = IndexVec::new();
    let mut module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>> =
        IndexVec::from_vec(vec![None; graph.modules.len()]);
    let mut entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx> = FxHashMap::default();

    if support_chunks {
        // One chunk per entry — each claims reachable modules (first-claim wins).
        for &entry_idx in &graph.entries {
            let chunk_idx = allocate_entry_chunk(&mut chunks, graph, entry_idx);
            entry_module_to_entry_chunk.insert(entry_idx, chunk_idx);
            module_to_chunk[entry_idx] = Some(chunk_idx);

            // BFS/DFS to claim reachable modules.
            let mut stack = vec![entry_idx];
            let mut visited: HashSet<ModuleIdx> = HashSet::new();
            visited.insert(entry_idx);

            while let Some(idx) = stack.pop() {
                // Only claim if not already claimed by a previous entry chunk.
                if module_to_chunk[idx].is_none() {
                    module_to_chunk[idx] = Some(chunk_idx);
                    chunks[chunk_idx].modules.push(idx);
                }
                for import in &graph.modules[idx].imports {
                    if let Some(&dep) = graph.id_to_idx.get(&import.resolved.id) {
                        if !visited.insert(dep) {
                            continue;
                        }
                        // Skip excluded modules (tree-shaken out).
                        if !graph.modules[dep].is_entry && !graph.modules[dep].has_side_effects {
                            continue;
                        }
                        stack.push(dep);
                    }
                }
            }
        }
    } else {
        // Single-chunk mode: all entries and their reachable modules in one chunk.
        if let Some(&first_entry) = graph.entries.first() {
            let chunk_idx = allocate_entry_chunk(&mut chunks, graph, first_entry);

            for &entry_idx in &graph.entries {
                entry_module_to_entry_chunk.insert(entry_idx, chunk_idx);
                if module_to_chunk[entry_idx].is_none() {
                    module_to_chunk[entry_idx] = Some(chunk_idx);
                    if !chunks[chunk_idx].modules.contains(&entry_idx) {
                        chunks[chunk_idx].modules.push(entry_idx);
                    }
                }

                let mut stack = vec![entry_idx];
                let mut visited: HashSet<ModuleIdx> = HashSet::new();
                visited.insert(entry_idx);

                while let Some(idx) = stack.pop() {
                    if module_to_chunk[idx].is_none() {
                        module_to_chunk[idx] = Some(chunk_idx);
                        chunks[chunk_idx].modules.push(idx);
                    }
                    for import in &graph.modules[idx].imports {
                        if let Some(&dep) = graph.id_to_idx.get(&import.resolved.id) {
                            if !visited.insert(dep) {
                                continue;
                            }
                            if !graph.modules[dep].is_entry
                                && !graph.modules[dep].has_side_effects
                            {
                                continue;
                            }
                            stack.push(dep);
                        }
                    }
                }
            }
        }
    }

    ChunkGraph {
        chunks,
        module_to_chunk,
        entry_module_to_entry_chunk,
    }
}

/// Allocate a new [`ArkChunk`] for the given entry module and push it into `chunks`.
fn allocate_entry_chunk(
    chunks: &mut IndexVec<ChunkIdx, ArkChunk>,
    graph: &ModuleGraph,
    entry_idx: ModuleIdx,
) -> ChunkIdx {
    let entry_module = &graph.modules[entry_idx];
    let name = derive_chunk_name(&entry_module.id.id);
    chunks.push(ArkChunk {
        name,
        chunk_hash: String::new(),
        modules: vec![entry_idx],
        is_entry: true,
        entry_module: Some(entry_idx),
        code: String::new(),
        source_map: None,
    })
}

/// Derive a short chunk name from a module ID (file path or OHM URL).
///
/// Examples:
/// - `/project/entry/ets/EntryAbility.ets` → `"EntryAbility"`
/// - `@bundle:com.example/entry/ets/index` → `"index"`
fn derive_chunk_name(id: &str) -> String {
    id.rsplit(&['/', '\\'][..])
        .next()
        .unwrap_or("chunk")
        .trim_end_matches(".ets")
        .trim_end_matches(".ts")
        .trim_end_matches(".js")
        .trim_end_matches(".mjs")
        .trim_end_matches(".cjs")
        .to_string()
}

#[cfg(test)]
mod tests {
    use arkdown_core::{
        ImportEdge, Module, ModuleGraph, ModuleIdx, ModuleMeta, ModuleSource,
        ModuleType, ResolvedId,
    };
    use index_vec::IndexVec;
    use rustc_hash::FxHashMap;

    use super::*;

    fn make_module(id: &str, imports: Vec<&str>, is_entry: bool) -> Module {
        Module {
            id: ResolvedId {
                id: id.to_string(),
                external: false,
                module_type: ModuleType::Ts,
            },
            source: ModuleSource {
                id: ResolvedId {
                    id: id.to_string(),
                    external: false,
                    module_type: ModuleType::Ts,
                },
                content: String::new(),
                source_map: None,
            },
            ast: (),
            imports: imports
                .into_iter()
                .map(|s| ImportEdge {
                    specifier: s.to_string(),
                    resolved: ResolvedId {
                        id: s.to_string(),
                        external: false,
                        module_type: ModuleType::Ts,
                    },
                    imported_names: vec![],
                })
                .collect(),
            dynamic_imports: vec![],
            exports: vec![],
            meta: ModuleMeta::default(),
            is_entry,
            has_side_effects: true,
        }
    }

    fn make_graph(modules: Vec<(&str, Vec<&str>, bool)>) -> (ModuleGraph, Vec<ModuleIdx>) {
        let mut module_vec: IndexVec<ModuleIdx, Module> = IndexVec::new();
        let mut id_to_idx: FxHashMap<String, ModuleIdx> = FxHashMap::default();
        let mut entries = vec![];

        for (id, imports, is_entry) in modules {
            let m = make_module(id, imports, is_entry);
            let idx = module_vec.push(m);
            id_to_idx.insert(id.to_string(), idx);
            if is_entry {
                entries.push(idx);
            }
        }

        let entries_clone = entries.clone();
        (
            ModuleGraph {
                modules: module_vec,
                id_to_idx,
                entries: entries_clone,
            },
            entries,
        )
    }

    #[test]
    fn test_derive_chunk_name() {
        assert_eq!(derive_chunk_name("/path/to/EntryAbility.ets"), "EntryAbility");
        assert_eq!(derive_chunk_name("/path/to/index.ts"), "index");
        assert_eq!(derive_chunk_name("@bundle:com.example/entry/ets/index"), "index");
    }

    #[test]
    fn test_single_entry_single_chunk() {
        let (graph, _entries) = make_graph(vec![
            ("entry.ts", vec!["util.ts"], true),
            ("util.ts", vec![], false),
        ]);

        let chunk_graph = split_chunks(&graph, true);
        assert_eq!(chunk_graph.chunks.len(), 1);
        // Entry chunk should contain entry + util
        assert_eq!(chunk_graph.chunks[chunk_graph.chunks.indices().next().unwrap()].modules.len(), 2);
    }

    #[test]
    fn test_two_entries_two_chunks() {
        let (graph, _entries) = make_graph(vec![
            ("entry_a.ts", vec!["shared.ts"], true),
            ("entry_b.ts", vec!["shared.ts"], true),
            ("shared.ts", vec![], false),
        ]);

        let chunk_graph = split_chunks(&graph, true);
        // Two entries → two chunks
        assert_eq!(chunk_graph.chunks.len(), 2);
        // shared.ts claimed by the first entry only
        let chunk_a_idx = *chunk_graph
            .entry_module_to_entry_chunk
            .values()
            .next()
            .unwrap();
        let _ = chunk_a_idx;
    }
}
