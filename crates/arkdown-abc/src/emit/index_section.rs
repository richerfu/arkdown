use rustc_hash::FxHashMap;

use crate::emit::bytecode::{collect_bytecode_index_dependencies, BytecodeIndexDependency};
use crate::emit::file::LiteralArray;
use crate::emit::record::ClassRecord;
use crate::emit::strings::StringTable;
use crate::names::{full_method_name, literal_array_key, short_method_name};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct MethodKey {
    pub(super) class_idx: usize,
    pub(super) method_idx: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MethodIndexItem {
    String(u16),
    Method(MethodKey),
    LiteralArray(usize),
}

#[derive(Debug, Default)]
pub(super) struct MethodBytecodeIndexPlan {
    pub(super) string_pool_indices: FxHashMap<u16, u16>,
    pub(super) literal_pool_indices: FxHashMap<u16, u16>,
    pub(super) method_pool_indices: FxHashMap<u16, u16>,
}

#[derive(Debug)]
pub(super) struct IndexSectionPlan {
    pub(super) items: Vec<MethodIndexItem>,
    pub(super) method_plans: Vec<Vec<MethodBytecodeIndexPlan>>,
}

impl IndexSectionPlan {
    pub(super) fn index_entry_count(&self) -> usize {
        self.items.len()
    }

    pub(super) fn method_plan(
        &self,
        class_idx: usize,
        method_idx: usize,
    ) -> &MethodBytecodeIndexPlan {
        &self.method_plans[class_idx][method_idx]
    }
}

pub(super) struct IndexSectionPlanner<'a> {
    strings: &'a StringTable,
    literal_arrays: &'a [LiteralArray],
}

impl<'a> IndexSectionPlanner<'a> {
    pub(super) fn new(strings: &'a StringTable, literal_arrays: &'a [LiteralArray]) -> Self {
        Self {
            strings,
            literal_arrays,
        }
    }

    pub(super) fn build(self, classes: &[&ClassRecord]) -> IndexSectionPlan {
        let local_methods_by_name = local_methods_by_name(classes);
        let methods_by_full_name = methods_by_full_name(classes);

        let mut raw_method_deps = classes
            .iter()
            .map(|class| {
                (0..class.methods.len())
                    .map(|_| Vec::<ResolvedIndexDependency>::new())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut string_ids = Vec::new();
        let mut method_keys = Vec::new();
        let mut literal_indices = Vec::new();
        let mut literal_owner_class = FxHashMap::<usize, usize>::default();

        for (class_idx, class) in classes.iter().enumerate() {
            for (method_idx, method) in class.methods.iter().enumerate() {
                for dep in collect_bytecode_index_dependencies(&method.bytecode) {
                    let resolved = match dep {
                        BytecodeIndexDependency::String(string_id) => {
                            expect_string(self.strings, string_id, class, method_idx);
                            string_ids.push(string_id);
                            ResolvedIndexDependency::String(string_id)
                        }
                        BytecodeIndexDependency::Method(name_id) => {
                            let method_key = self.resolve_method_operand(
                                classes,
                                &local_methods_by_name,
                                &methods_by_full_name,
                                class_idx,
                                method_idx,
                                name_id,
                            );
                            method_keys.push(method_key);
                            ResolvedIndexDependency::Method {
                                name_id,
                                method_key,
                            }
                        }
                        BytecodeIndexDependency::LiteralArray(literal_idx) => {
                            let literal_idx = literal_idx as usize;
                            self.literal_arrays.get(literal_idx).unwrap_or_else(|| {
                                panic!(
                                    "unresolved literalarray_id {literal_idx} while collecting ABC index dependencies for {}::{}",
                                    class.name, class.methods[method_idx].name
                                )
                            });
                            literal_owner_class.entry(literal_idx).or_insert(class_idx);
                            literal_indices.push(literal_idx);
                            ResolvedIndexDependency::LiteralArray(literal_idx as u16)
                        }
                    };
                    raw_method_deps[class_idx][method_idx].push(resolved);
                }
            }
        }

        sort_dedup_string_ids(&mut string_ids, self.strings);
        sort_dedup_method_keys(&mut method_keys, classes);
        sort_dedup_literal_indices(
            &mut literal_indices,
            classes,
            self.literal_arrays,
            &literal_owner_class,
        );

        let mut items =
            Vec::with_capacity(string_ids.len() + method_keys.len() + literal_indices.len());
        let mut string_slots = FxHashMap::default();
        let mut method_slots = FxHashMap::default();
        let mut literal_slots = FxHashMap::default();

        for string_id in string_ids {
            let slot = index_slot(items.len());
            items.push(MethodIndexItem::String(string_id));
            string_slots.insert(string_id, slot);
        }
        for method_key in method_keys {
            let slot = index_slot(items.len());
            items.push(MethodIndexItem::Method(method_key));
            method_slots.insert(method_key, slot);
        }
        for literal_idx in literal_indices {
            let slot = index_slot(items.len());
            items.push(MethodIndexItem::LiteralArray(literal_idx));
            literal_slots.insert(literal_idx, slot);
        }

        let mut method_plans = classes
            .iter()
            .map(|class| {
                (0..class.methods.len())
                    .map(|_| MethodBytecodeIndexPlan::default())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        for (class_idx, class_deps) in raw_method_deps.iter().enumerate() {
            for (method_idx, deps) in class_deps.iter().enumerate() {
                let plan = &mut method_plans[class_idx][method_idx];
                for dep in deps {
                    match *dep {
                        ResolvedIndexDependency::String(string_id) => {
                            let slot = *string_slots
                                .get(&string_id)
                                .expect("string dependency must have an index slot");
                            plan.string_pool_indices.insert(string_id, slot);
                        }
                        ResolvedIndexDependency::Method {
                            name_id,
                            method_key,
                        } => {
                            let slot = *method_slots
                                .get(&method_key)
                                .expect("method dependency must have an index slot");
                            plan.method_pool_indices.insert(name_id, slot);
                        }
                        ResolvedIndexDependency::LiteralArray(literal_idx) => {
                            let slot = *literal_slots
                                .get(&(literal_idx as usize))
                                .expect("literal dependency must have an index slot");
                            plan.literal_pool_indices.insert(literal_idx, slot);
                        }
                    }
                }
            }
        }

        IndexSectionPlan {
            items,
            method_plans,
        }
    }

    fn resolve_method_operand(
        &self,
        classes: &[&ClassRecord],
        local_methods_by_name: &[FxHashMap<&str, usize>],
        methods_by_full_name: &FxHashMap<String, MethodKey>,
        class_idx: usize,
        method_idx: usize,
        name_id: u16,
    ) -> MethodKey {
        let class = classes[class_idx];
        let method = &class.methods[method_idx];
        let name = self.strings.get(name_id as u32).unwrap_or_else(|| {
            panic!(
                "unresolved method_id string index {name_id} while collecting ABC index dependencies for {}::{}",
                class.name, method.name
            )
        });

        if let Some(&target_method_idx) = local_methods_by_name[class_idx].get(name) {
            return MethodKey {
                class_idx,
                method_idx: target_method_idx,
            };
        }

        if let Some(&method_key) = methods_by_full_name.get(name) {
            return method_key;
        }

        let short_name = short_method_name(&class.name, name);
        if short_name != name {
            if let Some(&target_method_idx) =
                local_methods_by_name[class_idx].get(short_name.as_str())
            {
                return MethodKey {
                    class_idx,
                    method_idx: target_method_idx,
                };
            }
        }

        panic!(
            "unresolved method_id string index {name_id} ('{name}') while collecting ABC index dependencies for {}::{}",
            class.name, method.name
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResolvedIndexDependency {
    String(u16),
    Method { name_id: u16, method_key: MethodKey },
    LiteralArray(u16),
}

fn local_methods_by_name<'a>(classes: &'a [&'a ClassRecord]) -> Vec<FxHashMap<&'a str, usize>> {
    classes
        .iter()
        .map(|class| {
            class
                .methods
                .iter()
                .enumerate()
                .map(|(method_idx, method)| (method.name.as_str(), method_idx))
                .collect()
        })
        .collect()
}

fn methods_by_full_name(classes: &[&ClassRecord]) -> FxHashMap<String, MethodKey> {
    let mut methods = FxHashMap::default();
    for (class_idx, class) in classes.iter().enumerate() {
        for (method_idx, method) in class.methods.iter().enumerate() {
            methods.insert(
                full_method_name(&class.name, &method.name),
                MethodKey {
                    class_idx,
                    method_idx,
                },
            );
        }
    }
    methods
}

fn sort_dedup_string_ids(string_ids: &mut Vec<u16>, strings: &StringTable) {
    string_ids.sort_by(|a, b| {
        strings
            .get(*a as u32)
            .unwrap_or("")
            .cmp(strings.get(*b as u32).unwrap_or(""))
            .then_with(|| a.cmp(b))
    });
    string_ids.dedup();
}

fn sort_dedup_method_keys(method_keys: &mut Vec<MethodKey>, classes: &[&ClassRecord]) {
    method_keys.sort_by(|a, b| {
        let a_name = full_method_name(
            &classes[a.class_idx].name,
            &classes[a.class_idx].methods[a.method_idx].name,
        );
        let b_name = full_method_name(
            &classes[b.class_idx].name,
            &classes[b.class_idx].methods[b.method_idx].name,
        );
        a_name
            .cmp(&b_name)
            .then_with(|| a.class_idx.cmp(&b.class_idx))
            .then_with(|| a.method_idx.cmp(&b.method_idx))
    });
    method_keys.dedup();
}

fn sort_dedup_literal_indices(
    literal_indices: &mut Vec<usize>,
    classes: &[&ClassRecord],
    literal_arrays: &[LiteralArray],
    literal_owner_class: &FxHashMap<usize, usize>,
) {
    literal_indices.sort_by(|a, b| {
        literal_sort_key(*a, classes, literal_arrays, literal_owner_class)
            .cmp(&literal_sort_key(
                *b,
                classes,
                literal_arrays,
                literal_owner_class,
            ))
            .then_with(|| a.cmp(b))
    });
    literal_indices.dedup();
}

fn literal_sort_key(
    literal_idx: usize,
    classes: &[&ClassRecord],
    literal_arrays: &[LiteralArray],
    literal_owner_class: &FxHashMap<usize, usize>,
) -> String {
    literal_arrays
        .get(literal_idx)
        .and_then(|array| array.explicit_key.clone())
        .unwrap_or_else(|| {
            let class_idx = *literal_owner_class
                .get(&literal_idx)
                .expect("literal dependency must have an owner class");
            literal_array_key(&classes[class_idx].name, literal_idx)
        })
}

fn expect_string(strings: &StringTable, string_id: u16, class: &ClassRecord, method_idx: usize) {
    strings.get(string_id as u32).unwrap_or_else(|| {
        panic!(
            "unresolved string_id {string_id} while collecting ABC index dependencies for {}::{}",
            class.name, class.methods[method_idx].name
        )
    });
}

fn index_slot(len: usize) -> u16 {
    u16::try_from(len).expect("ABC IndexHeader method/string/literal index exceeds u16 range")
}
