use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rustc_hash::FxHashMap;

use crate::emit::bytecode::remap_literal_array_ids;
use crate::emit::file::{AbcFile, LiteralEntry};
use crate::emit::record::{ClassRecord, FieldValue};
use crate::ir::opcodes::Insn;

const NPM_ENTRIES: &str = "npmEntries.txt";
const IS_COMMONJS: &str = "isCommonjs";
const JSON_FILE_CONTENT: &str = "jsonFileContent";
const MODULE_RECORD_IDX: &str = "moduleRecordIdx";
const NORMALIZED_OHMURL_NOT_SO: &str = "@normalized:N";
const NORMALIZED_OHMURL_SEPARATOR: char = '&';
const NORMALIZED_OHMURL_PREFIX: char = '@';
const SLASH_TAG: char = '/';

const BUNDLE_NAME_POS: usize = 2;
const NORMALIZED_IMPORT_POS: usize = 3;
const VERSION_POS: usize = 4;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbcDependencyContext {
    pub compile_entries: Vec<String>,
    pub external_pkg_names: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedDepsRelation {
    programs: BTreeMap<String, BTreeSet<String>>,
}

impl ResolvedDepsRelation {
    pub(crate) fn contains_program(&self, program_key: &str) -> bool {
        self.programs.contains_key(program_key)
    }

    fn insert(&mut self, program_key: &str, record_name: &str) {
        self.programs
            .entry(program_key.to_string())
            .or_default()
            .insert(record_name.to_string());
    }

    fn dump(&self) {
        println!("All Dependency Files:");
        for (program_key, records) in &self.programs {
            println!("program_file: {}", display_program_file(program_key));
            for record in records {
                println!("record_name: {record}");
            }
        }
        println!();
    }
}

pub(crate) fn resolve_deps_relation(
    file: &AbcFile,
    record_program_keys: &[String],
    context: &AbcDependencyContext,
    dump_deps_info: bool,
) -> ResolvedDepsRelation {
    let mut resolver = DepsRelationResolver {
        file,
        record_program_keys,
        context,
        resolved: ResolvedDepsRelation::default(),
        deps_to_be_resolved: VecDeque::new(),
        resolved_records: BTreeSet::new(),
    };
    resolver.resolve();
    if dump_deps_info {
        resolver.resolved.dump();
    }
    resolver.resolved
}

pub(crate) fn compact_to_retained_programs(
    file: &mut AbcFile,
    record_program_keys: &mut Vec<String>,
    keep_mask: &[bool],
) -> BTreeMap<usize, usize> {
    let old_records = std::mem::take(&mut file.records);
    let old_program_keys = std::mem::take(record_program_keys);

    let mut record_index_map = BTreeMap::new();
    let mut retained_records = Vec::new();
    let mut retained_program_keys = Vec::new();
    for (old_index, (record, program_key)) in
        old_records.into_iter().zip(old_program_keys).enumerate()
    {
        if !keep_mask.get(old_index).copied().unwrap_or(false) {
            continue;
        }
        record_index_map.insert(old_index, retained_records.len());
        retained_records.push(record);
        retained_program_keys.push(program_key);
    }

    let literal_index_map = compact_literal_arrays(file, &retained_records);
    for record in &mut retained_records {
        remap_record_literal_ids(record, &literal_index_map);
    }

    file.records = retained_records;
    *record_program_keys = retained_program_keys;
    record_index_map
}

struct DepsRelationResolver<'a> {
    file: &'a AbcFile,
    record_program_keys: &'a [String],
    context: &'a AbcDependencyContext,
    resolved: ResolvedDepsRelation,
    deps_to_be_resolved: VecDeque<String>,
    resolved_records: BTreeSet<String>,
}

impl DepsRelationResolver<'_> {
    fn resolve(&mut self) {
        let record_to_program = self.fill_record_to_program_map();

        for entry_record in &self.context.compile_entries {
            self.deps_to_be_resolved.push_back(entry_record.clone());
            self.resolved_records.insert(entry_record.clone());

            while let Some(record_name) = self.deps_to_be_resolved.pop_front() {
                let Some(program_key) = record_to_program.get(&record_name) else {
                    continue;
                };
                self.resolved.insert(program_key, &record_name);
                self.collect_static_import_deps_relation(&record_name);
                self.collect_dynamic_import_deps_relation(&record_name);
            }
        }
    }

    fn fill_record_to_program_map(&mut self) -> BTreeMap<String, String> {
        let mut record_to_program = BTreeMap::new();
        for (record_index, record) in self.file.records.iter().enumerate() {
            let Some(program_key) = self.record_program_keys.get(record_index) else {
                continue;
            };
            if record.fields.is_empty() {
                continue;
            }
            let record_name = dependency_record_name(&record.name);
            if program_key.contains(NPM_ENTRIES) {
                self.resolved.insert(program_key, &record_name);
                continue;
            }
            if self.collect_commonjs_and_json_record(record, program_key) {
                continue;
            }
            record_to_program.insert(record.name.clone(), program_key.clone());
            record_to_program.insert(record_name, program_key.clone());
        }
        record_to_program
    }

    fn collect_commonjs_and_json_record(
        &mut self,
        record: &ClassRecord,
        program_key: &str,
    ) -> bool {
        let record_name = dependency_record_name(&record.name);
        for field in &record.fields {
            if field.name == JSON_FILE_CONTENT {
                self.resolved.insert(program_key, &record_name);
                return true;
            }
            if !field.name.contains(IS_COMMONJS) {
                continue;
            }
            if matches!(field.value, Some(FieldValue::U8(value)) if value > 0) {
                self.resolved.insert(program_key, &record_name);
                return true;
            }
        }
        false
    }

    fn collect_static_import_deps_relation(&mut self, record_name: &str) {
        let mut imports = Vec::new();
        for record in &self.file.records {
            if !record.name.contains(record_name) {
                continue;
            }
            imports.extend(static_imports(self.file, record));
        }
        for ohmurl in imports {
            self.collect_deps_if_needed(&ohmurl);
        }
    }

    fn collect_dynamic_import_deps_relation(&mut self, record_name: &str) {
        let mut imports = Vec::new();
        for record in &self.file.records {
            for method in &record.methods {
                if !method.name.contains(record_name) {
                    continue;
                }
                imports.extend(dynamic_imports(self.file, &method.pandasm_insns));
            }
        }
        for ohmurl in imports {
            self.collect_deps_if_needed(&ohmurl);
        }
    }

    fn collect_deps_if_needed(&mut self, ohmurl: &str) {
        if !ohmurl.contains(NORMALIZED_OHMURL_NOT_SO)
            || is_external_pkg_name(ohmurl, &self.context.external_pkg_names)
        {
            return;
        }
        let Some(record_name) = record_name_from_normalized_ohmurl(ohmurl) else {
            return;
        };
        if self.resolved_records.insert(record_name.clone()) {
            self.deps_to_be_resolved.push_back(record_name);
        }
    }
}

fn static_imports(file: &AbcFile, record: &ClassRecord) -> Vec<String> {
    if !record.module_requests.is_empty() {
        return record.module_requests.clone();
    }

    let Some(module_record_idx) = record.fields.iter().find_map(|field| {
        (field.name == MODULE_RECORD_IDX)
            .then_some(field.value)
            .flatten()
            .and_then(|value| match value {
                FieldValue::LiteralArray(idx) => Some(idx),
                FieldValue::U8(_) | FieldValue::U32(_) => None,
            })
    }) else {
        return Vec::new();
    };
    let Some(array) = file.literal_arrays.get(module_record_idx) else {
        return Vec::new();
    };
    let Some(import_count) = array.entries.first().and_then(raw_u32) else {
        return Vec::new();
    };
    array
        .entries
        .iter()
        .skip(1)
        .take(import_count as usize)
        .filter_map(raw_string)
        .map(str::to_string)
        .collect()
}

fn dynamic_imports(file: &AbcFile, insns: &[Insn]) -> Vec<String> {
    let mut imports = Vec::new();
    for window in insns.windows(2) {
        let [Insn::LdaStr(string_id), Insn::DynamicImport] = window else {
            continue;
        };
        if let Some(value) = file.strings.get(u32::from(*string_id)) {
            imports.push(value.to_string());
        }
    }
    imports
}

fn compact_literal_arrays(
    file: &mut AbcFile,
    retained_records: &[ClassRecord],
) -> BTreeMap<usize, usize> {
    let mut used = BTreeSet::new();
    for record in retained_records {
        used.extend(record.literal_indices.iter().copied());
        used.extend(record.module_literal_indices.iter().copied());
        for field in &record.fields {
            if let Some(FieldValue::LiteralArray(idx)) = field.value {
                used.insert(idx);
            }
        }
    }

    let old_literals = std::mem::take(&mut file.literal_arrays);
    let mut literal_index_map = BTreeMap::new();
    for (old_index, literal) in old_literals.into_iter().enumerate() {
        if !used.contains(&old_index) {
            continue;
        }
        literal_index_map.insert(old_index, file.literal_arrays.len());
        file.literal_arrays.push(literal);
    }
    literal_index_map
}

fn remap_record_literal_ids(record: &mut ClassRecord, literal_index_map: &BTreeMap<usize, usize>) {
    for field in &mut record.fields {
        if let Some(FieldValue::LiteralArray(idx)) = field.value {
            if let Some(&new_idx) = literal_index_map.get(&idx) {
                field.value = Some(FieldValue::LiteralArray(new_idx));
            }
        }
    }
    remap_literal_index_vec(&mut record.literal_indices, literal_index_map);
    remap_literal_index_vec(&mut record.module_literal_indices, literal_index_map);

    let bytecode_map = literal_index_map
        .iter()
        .map(|(&old_idx, &new_idx)| (old_idx as u16, new_idx as u16))
        .collect::<FxHashMap<_, _>>();
    for method in &mut record.methods {
        remap_literal_insns(&mut method.pandasm_insns, &bytecode_map);
        remap_literal_array_ids(&mut method.bytecode, &bytecode_map);
    }
}

fn remap_literal_index_vec(values: &mut Vec<usize>, literal_index_map: &BTreeMap<usize, usize>) {
    values.retain_mut(|idx| {
        let Some(&new_idx) = literal_index_map.get(idx) else {
            return false;
        };
        *idx = new_idx;
        true
    });
}

fn remap_literal_insns(insns: &mut [Insn], literal_index_map: &FxHashMap<u16, u16>) {
    for insn in insns {
        match insn {
            Insn::NewLexEnvWithName(_, literal_id)
            | Insn::CallRuntimeCreatePrivateProperty(_, literal_id)
            | Insn::DefineClassWithBuffer(_, _, literal_id, _, _)
            | Insn::CreateArrayWithBuffer(_, literal_id)
            | Insn::CreateObjectWithBuffer(_, literal_id) => {
                if let Some(&new_id) = literal_index_map.get(literal_id) {
                    *literal_id = new_id;
                }
            }
            _ => {}
        }
    }
}

fn raw_u32(entry: &LiteralEntry) -> Option<u32> {
    match entry {
        LiteralEntry::RawU32(value) => Some(*value),
        _ => None,
    }
}

fn raw_string(entry: &LiteralEntry) -> Option<&str> {
    match entry {
        LiteralEntry::RawString(value) => Some(value),
        _ => None,
    }
}

fn record_name_from_normalized_ohmurl(ohmurl: &str) -> Option<String> {
    let items = ohmurl
        .split(NORMALIZED_OHMURL_SEPARATOR)
        .collect::<Vec<_>>();
    if items.len() <= VERSION_POS {
        return None;
    }
    Some(format!(
        "{}{}{}{}{}",
        items[BUNDLE_NAME_POS],
        NORMALIZED_OHMURL_SEPARATOR,
        items[NORMALIZED_IMPORT_POS],
        NORMALIZED_OHMURL_SEPARATOR,
        items[VERSION_POS]
    ))
}

fn is_external_pkg_name(ohmurl: &str, external_pkg_names: &BTreeSet<String>) -> bool {
    let Some(pkg_name) = pkg_name_from_normalized_ohmurl(ohmurl) else {
        return false;
    };
    external_pkg_names.contains(&pkg_name)
}

fn pkg_name_from_normalized_ohmurl(ohmurl: &str) -> Option<String> {
    let items = ohmurl
        .split(NORMALIZED_OHMURL_SEPARATOR)
        .collect::<Vec<_>>();
    let normalized_import = items.get(NORMALIZED_IMPORT_POS)?;
    pkg_name_from_normalized_import(normalized_import)
}

fn pkg_name_from_normalized_import(normalized_import: &str) -> Option<String> {
    let mut pos = normalized_import.find(SLASH_TAG)?;
    if normalized_import.starts_with(NORMALIZED_OHMURL_PREFIX) {
        let next = normalized_import[pos + 1..].find(SLASH_TAG)?;
        pos += 1 + next;
    }
    Some(normalized_import[..pos].to_string())
}

fn display_program_file(program_key: &str) -> &str {
    let abc_separator = program_key.rfind('|').unwrap_or(program_key.len());
    let Some(slash_pos) = program_key[..abc_separator].rfind('/') else {
        return program_key;
    };
    &program_key[slash_pos + 1..abc_separator]
}

fn dependency_record_name(class_name: &str) -> String {
    let Some(stripped) = class_name
        .strip_prefix("L_")
        .or_else(|| class_name.strip_prefix('L'))
    else {
        return class_name.to_string();
    };
    stripped.strip_suffix(';').unwrap_or(stripped).to_string()
}
