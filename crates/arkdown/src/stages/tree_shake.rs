use std::collections::HashSet;

use arkdown_core::{ModuleGraph, ModuleIdx};

/// Mark modules as excluded based on reachability from entry points.
///
/// Performs a simple BFS/DFS reachability analysis from the entry modules.
/// Modules not reachable from any entry are marked by clearing their
/// `has_side_effects` flag, signaling to `chunk_split` that they can be
/// omitted from the output.
///
/// Note: This is a conservative implementation. A full binding-level
/// tree-shaker (like rolldown's) would require complete symbol resolution
/// from the `link` stage. For now we do module-level reachability only.
pub fn tree_shake(graph: &mut ModuleGraph) {
    let mut included: HashSet<ModuleIdx> = HashSet::new();
    let mut stack: Vec<ModuleIdx> = graph.entries.clone();

    while let Some(idx) = stack.pop() {
        if included.insert(idx) {
            for import in graph.modules[idx].imports() {
                if let Some(&dep_idx) = graph.id_to_idx.get(&import.resolved.id) {
                    if !included.contains(&dep_idx) {
                        stack.push(dep_idx);
                    }
                }
            }
        }
    }

    // Modules not reachable from any entry are excluded from chunks.
    // We mark them by clearing `has_side_effects` so chunk_split can skip them.
    for (idx, module) in graph.modules.iter_mut_enumerated() {
        if !included.contains(&idx) {
            if let Some(nm) = module.as_normal_mut() {
                nm.has_side_effects = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use arkdown_core::{
        ImportEdge, ImportKind, Module, ModuleGraph, ModuleIdx, ModuleMeta, ModuleSource,
        ModuleType, NormalModule, ResolvedId,
    };
    use index_vec::IndexVec;
    use rustc_hash::FxHashMap;

    fn make_module(id: &str, imports: Vec<&str>) -> Module {
        Module::Normal(Box::new(NormalModule {
            idx: ModuleIdx(0),
            exec_order: 0,
            id: ResolvedId {
                id: id.to_string(),
                external: false,
                module_type: ModuleType::Ts,
                ..Default::default()
            },
            source: ModuleSource {
                id: ResolvedId {
                    id: id.to_string(),
                    external: false,
                    module_type: ModuleType::Ts,
                    ..Default::default()
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
                        ..Default::default()
                    },
                    imported_names: vec![],
                    kind: ImportKind::Import,
                })
                .collect(),
            dynamic_imports: vec![],
            exports: vec![],
            meta: ModuleMeta::default(),
            is_entry: false,
            has_side_effects: true,
        }))
    }

    #[test]
    fn test_tree_shake_excludes_unreachable() {
        let mut modules: IndexVec<ModuleIdx, Module> = IndexVec::new();
        let mut id_to_idx: FxHashMap<String, ModuleIdx> = FxHashMap::default();

        // entry → a → b; c is unreachable
        let entry_idx = modules.push(make_module("entry", vec!["a"]));
        id_to_idx.insert("entry".to_string(), entry_idx);

        let a_idx = modules.push(make_module("a", vec!["b"]));
        id_to_idx.insert("a".to_string(), a_idx);

        let b_idx = modules.push(make_module("b", vec![]));
        id_to_idx.insert("b".to_string(), b_idx);

        let c_idx = modules.push(make_module("c", vec![]));
        id_to_idx.insert("c".to_string(), c_idx);

        let mut graph = ModuleGraph {
            modules,
            id_to_idx,
            entries: vec![entry_idx],
        };

        super::tree_shake(&mut graph);

        // entry, a, b are included → has_side_effects left as-is (true)
        // c is excluded → has_side_effects set to false
        assert!(graph.modules[entry_idx].has_side_effects());
        assert!(graph.modules[a_idx].has_side_effects());
        assert!(graph.modules[b_idx].has_side_effects());
        assert!(!graph.modules[c_idx].has_side_effects());
    }

    /// An entry module retains its `has_side_effects` value regardless of what
    /// it was set to before tree-shaking — entry modules are always reachable.
    #[test]
    fn test_entry_module_always_retained() {
        let mut modules: IndexVec<ModuleIdx, Module> = IndexVec::new();
        let mut id_to_idx: FxHashMap<String, ModuleIdx> = FxHashMap::default();

        // Build a module that has_side_effects = true (default from make_module).
        // Tree-shaking should leave it as-is because it is the entry.
        let entry_idx = modules.push(make_module("entry_only", vec![]));
        id_to_idx.insert("entry_only".to_string(), entry_idx);

        let mut graph = ModuleGraph {
            modules,
            id_to_idx,
            entries: vec![entry_idx],
        };

        // Confirm starting state.
        assert!(graph.modules[entry_idx].has_side_effects());

        super::tree_shake(&mut graph);

        // Entry is in the included set, so its has_side_effects flag is not cleared.
        assert!(
            graph.modules[entry_idx].has_side_effects(),
            "entry module must never be excluded by tree-shake"
        );
    }

    /// A module reachable from the entry (even transitively) must be retained.
    #[test]
    fn test_imported_module_retained() {
        let mut modules: IndexVec<ModuleIdx, Module> = IndexVec::new();
        let mut id_to_idx: FxHashMap<String, ModuleIdx> = FxHashMap::default();

        // chain: entry → imported → leaf
        let entry_idx = modules.push(make_module("entry", vec!["imported"]));
        id_to_idx.insert("entry".to_string(), entry_idx);

        let imported_idx = modules.push(make_module("imported", vec!["leaf"]));
        id_to_idx.insert("imported".to_string(), imported_idx);

        let leaf_idx = modules.push(make_module("leaf", vec![]));
        id_to_idx.insert("leaf".to_string(), leaf_idx);

        let mut graph = ModuleGraph {
            modules,
            id_to_idx,
            entries: vec![entry_idx],
        };

        super::tree_shake(&mut graph);

        assert!(
            graph.modules[imported_idx].has_side_effects(),
            "module imported by entry must be retained"
        );
        assert!(
            graph.modules[leaf_idx].has_side_effects(),
            "transitively imported module must be retained"
        );
    }

    /// Multiple unreachable modules are all excluded in one pass.
    #[test]
    fn test_multiple_unreachable_modules_excluded() {
        let mut modules: IndexVec<ModuleIdx, Module> = IndexVec::new();
        let mut id_to_idx: FxHashMap<String, ModuleIdx> = FxHashMap::default();

        let entry_idx = modules.push(make_module("entry", vec![]));
        id_to_idx.insert("entry".to_string(), entry_idx);

        // Three unreachable modules.
        let orphan_a = modules.push(make_module("orphan_a", vec![]));
        id_to_idx.insert("orphan_a".to_string(), orphan_a);
        let orphan_b = modules.push(make_module("orphan_b", vec![]));
        id_to_idx.insert("orphan_b".to_string(), orphan_b);
        let orphan_c = modules.push(make_module("orphan_c", vec![]));
        id_to_idx.insert("orphan_c".to_string(), orphan_c);

        let mut graph = ModuleGraph {
            modules,
            id_to_idx,
            entries: vec![entry_idx],
        };

        super::tree_shake(&mut graph);

        assert!(!graph.modules[orphan_a].has_side_effects());
        assert!(!graph.modules[orphan_b].has_side_effects());
        assert!(!graph.modules[orphan_c].has_side_effects());
    }
}
