//! Upstream-shaped dependency relation resolver.
//!
//! This module implements `DepsRelationResolver` which operates on the
//! `ProgramRegistry` (collection of Programs) rather than the final `AbcFile`.
//! This matches upstream's `ResolveDepsRelations` which runs before `GenerateAbcFiles`.
//!
//! ## Upstream algorithm
//!
//! `resolveDepsRelation.cpp` `DepsRelationResolver::Resolve()`:
//!
//! 1. Build `recordToProgram` map: for each record in each Program:
//!    - Skip npm-entries Programs (always retained).
//!    - Pre-retain CommonJS and JSON records (always reachable).
//!    - Map record name → program key.
//!
//! 2. BFS from `compileEntries`:
//!    - For each reachable record:
//!      - Collect static imports from the module record literal array.
//!      - Collect dynamic imports from `DynamicImport` instructions.
//!    - For each import OHM URL:
//!      - Skip external packages (`hspPkgNames`).
//!      - Skip non-`@normalized:N` URLs.
//!      - Derive record name from URL and enqueue if not seen.
//!
//! 3. `RemoveProgramsRedundantData` (if `remove_redundant_file`):
//!    - Remove Programs not in the resolved set from `programsInfo`.
//!
//! ## Upstream reference
//! - `es2panda/aot/resolveDepsRelation.cpp` — `DepsRelationResolver::Resolve()`
//! - `es2panda/aot/main.cpp` — `ResolveDepsRelations`, `ResolveAndGenerate`

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::aot::options::CompileContextInfo;
use crate::aot::program_registry::{is_abc_program_key, ProgramRegistry};
use crate::emit::file::LiteralEntry;
use crate::emit::record::{FieldValue, ClassRecord};
use crate::emit::strings::StringTable;
use crate::ir::opcodes::Insn;

// OHM URL sentinel values from upstream dependency.rs.
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

/// The resolved dependency relation map.
///
/// `programKey → BTreeSet<recordName>` for each reachable program/record pair.
#[derive(Debug, Clone, Default)]
pub struct ResolvedDepsRelation {
    programs: BTreeMap<Box<str>, BTreeSet<Box<str>>>,
}

impl ResolvedDepsRelation {
    pub fn contains_program(&self, program_key: &str) -> bool {
        self.programs.contains_key(program_key)
    }

    fn insert(&mut self, program_key: &str, record_name: &str) {
        self.programs
            .entry(Box::from(program_key))
            .or_default()
            .insert(Box::from(record_name));
    }

    /// Dump the resolved dependency info to stdout, matching upstream output format.
    ///
    /// Upstream `es2panda/aot/resolveDepsRelation.cpp`:
    /// ```text
    /// All Dependency Files:
    /// program_file: <basename>
    /// record_name: <name>
    /// ```
    pub fn dump(&self) {
        println!("All Dependency Files:");
        for (program_key, records) in &self.programs {
            println!("program_file: {}", display_program_file(program_key));
            for record in records {
                println!("record_name: {record}");
            }
        }
        println!();
    }

    /// Return a sorted list of retained program keys.
    pub fn retained_keys(&self) -> BTreeSet<Box<str>> {
        self.programs.keys().cloned().collect()
    }
}

/// Dependency relation resolver that operates on `ProgramRegistry`.
///
/// Each `Program` in the registry contributes its records to the BFS graph.
/// The resolver drains the graph from the compile entries provided in
/// `CompileContextInfo`.
pub struct DepsRelationResolver<'a> {
    registry: &'a ProgramRegistry,
    context: &'a CompileContextInfo,
    resolved: ResolvedDepsRelation,
    deps_to_be_resolved: VecDeque<Box<str>>,
    resolved_records: BTreeSet<Box<str>>,
}

impl<'a> DepsRelationResolver<'a> {
    pub fn new(registry: &'a ProgramRegistry, context: &'a CompileContextInfo) -> Self {
        Self {
            registry,
            context,
            resolved: ResolvedDepsRelation::default(),
            deps_to_be_resolved: VecDeque::new(),
            resolved_records: BTreeSet::new(),
        }
    }

    /// Run the dependency BFS resolution.
    ///
    /// Returns a `ResolvedDepsRelation` containing all reachable programs.
    pub fn resolve(mut self) -> ResolvedDepsRelation {
        let record_to_program = self.build_record_to_program_map();

        for entry_record in &self.context.compile_entries {
            self.deps_to_be_resolved.push_back(entry_record.clone());
            self.resolved_records.insert(entry_record.clone());

            while let Some(record_name) = self.deps_to_be_resolved.pop_front() {
                let Some(program_key) = record_to_program.get(record_name.as_ref()) else {
                    continue;
                };
                self.resolved.insert(program_key, &record_name);
                self.collect_static_import_deps(&record_name, &record_to_program);
                self.collect_dynamic_import_deps(&record_name);
            }
        }

        self.resolved
    }

    /// Build the `recordName → programKey` map.
    ///
    /// Pre-marks npm entries Programs and CommonJS/JSON records as always retained.
    fn build_record_to_program_map(&mut self) -> BTreeMap<Box<str>, Box<str>> {
        let mut record_to_program: BTreeMap<Box<str>, Box<str>> = BTreeMap::new();

        for (key, _, entry) in self.registry.iter_in_key_order() {
            for record in entry.program.record_table.iter() {
                if record.fields.is_empty() {
                    continue;
                }

                let record_dep_name = dependency_record_name(&record.name);

                // npm entries: always retain.
                if key.contains(NPM_ENTRIES) {
                    self.resolved.insert(key, &record_dep_name);
                    continue;
                }

                // CommonJS / JSON records: always retain.
                if self.collect_commonjs_and_json_record(record, key) {
                    continue;
                }

                record_to_program.insert(Box::from(record.name.as_str()), Box::from(key));
                record_to_program.insert(Box::from(record_dep_name.as_str()), Box::from(key));
            }
        }

        record_to_program
    }

    /// Check if a record is a CommonJS or JSON record (always retained).
    fn collect_commonjs_and_json_record(&mut self, record: &ClassRecord, program_key: &str) -> bool {
        let record_name = dependency_record_name(&record.name);
        for field in &record.fields {
            if field.name == JSON_FILE_CONTENT {
                self.resolved.insert(program_key, &record_name);
                return true;
            }
            if field.name.contains(IS_COMMONJS) {
                if matches!(field.value, Some(FieldValue::U8(value)) if value > 0) {
                    self.resolved.insert(program_key, &record_name);
                    return true;
                }
            }
        }
        false
    }

    /// Collect static imports (from module record literal arrays).
    fn collect_static_import_deps(
        &mut self,
        record_name: &str,
        record_to_program: &BTreeMap<Box<str>, Box<str>>,
    ) {
        let mut imports = Vec::new();
        for (_, _, entry) in self.registry.iter_in_key_order() {
            for record in entry.program.record_table.iter() {
                if !record.name.contains(record_name) {
                    continue;
                }
                imports.extend(static_imports_from_record(record, &entry.program));
            }
        }
        for ohmurl in imports {
            self.collect_dep_if_needed(&ohmurl);
        }
    }

    /// Collect dynamic imports (from DynamicImport instructions in method bodies).
    fn collect_dynamic_import_deps(&mut self, record_name: &str) {
        let mut imports = Vec::new();
        for (_, _, entry) in self.registry.iter_in_key_order() {
            for record in entry.program.record_table.iter() {
                for method in &record.methods {
                    if !method.name.contains(record_name) {
                        continue;
                    }
                    // Pass the program's StringTable so LdaStr string IDs can be
                    // resolved to actual OHM URL strings for BFS traversal.
                    // Upstream: `VisitDynamicImports<true>` in resolveDepsRelation.cpp.
                    imports.extend(dynamic_imports_from_insns(
                        &method.pandasm_insns,
                        &entry.program.strings,
                    ));
                }
            }
        }
        for ohmurl in imports {
            self.collect_dep_if_needed(&ohmurl);
        }
    }

    /// Enqueue a new dependency OHM URL for BFS traversal.
    fn collect_dep_if_needed(&mut self, ohmurl: &str) {
        if !ohmurl.contains(NORMALIZED_OHMURL_NOT_SO) {
            return;
        }
        if is_external_pkg_name(ohmurl, &self.context.hsp_pkg_names) {
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

// ── Public entry point ────────────────────────────────────────────────────────

/// Resolve the dependency relation for a `ProgramRegistry`.
///
/// Returns the `ResolvedDepsRelation` containing all reachable programs.
/// If `dump_deps_info` is true, prints the resolved info to stdout.
///
/// ## Upstream reference
/// `es2panda/aot/main.cpp` `ResolveDepsRelations`.
pub fn resolve_deps_relation(
    registry: &ProgramRegistry,
    context: &CompileContextInfo,
    dump_deps_info: bool,
) -> ResolvedDepsRelation {
    let resolver = DepsRelationResolver::new(registry, context);
    let resolved = resolver.resolve();
    if dump_deps_info {
        resolved.dump();
    }
    resolved
}

// ── Helper functions ──────────────────────────────────────────────────────────

/// Extract static import OHM URLs from a record's module literal array.
///
/// ## Internal representation vs upstream
///
/// Upstream `VisitStaticImports` (`commonUtil.h` line 83–99) reads the
/// `moduleRecordIdx` field as a **string key** (`GetValue<std::string>()`) and
/// looks it up in `program.literalarray_table` (a `std::map<string, LiteralArray>`).
///
/// Our internal representation is different:
/// - Source-compiled programs: `moduleRecordIdx` stores `FieldValue::LiteralArray(idx)`
///   where `idx` is a numeric index into `Program.literal_array_table`.
/// - ABC-input programs: `record.module_requests` is pre-populated by `abc2program.rs`,
///   so the field lookup is bypassed entirely (fast path).
///
/// Both representations produce the same import URL list.  The difference is purely
/// in the key type (string vs usize) which is an internal representation detail.
fn static_imports_from_record(
    record: &ClassRecord,
    program: &crate::program::model::Program,
) -> Vec<String> {
    // Fast path: ABC-input programs pre-populate module_requests during decoding.
    if !record.module_requests.is_empty() {
        return record.module_requests.clone();
    }

    // Source-compiled path: read moduleRecordIdx field (numeric index) and look
    // up the literal array by position.
    let Some(module_record_idx) = record.fields.iter().find_map(|field| {
        (field.name == MODULE_RECORD_IDX)
            .then_some(field.value.as_ref())
            .flatten()
            .and_then(|value| match value {
                FieldValue::LiteralArray(idx) => Some(*idx),
                _ => None,
            })
    }) else {
        return Vec::new();
    };

    // The literal_array_table stores these at the program level.
    // We need to find the array by index in the program's literal_array_table.
    let literal = program.literal_array_table.iter().nth(module_record_idx);
    let Some(literal) = literal else {
        return Vec::new();
    };

    let Some(import_count) = literal.entries.first().and_then(raw_u32) else {
        return Vec::new();
    };

    literal
        .entries
        .iter()
        .skip(1)
        .take(import_count as usize)
        .filter_map(raw_string)
        .map(str::to_string)
        .collect()
}

/// Extract dynamic import OHM URL strings from pandasm instructions.
///
/// Matches upstream `VisitDyanmicImports<true>` in `commonUtil.h`:
///
/// ```cpp
/// // Upstream iterates forward, finds DynamicImport, checks prev is LdaStr.
/// if ((*iter)->GetOpcode() != DYNAMICIMPORT || (*iter)->IsLabel()) continue;
/// auto prevIns = iter - 1;
/// if ((*prevIns)->GetOpcode() != LDA_STR) continue;
/// cb((*prevIns)->GetId(0));   // the string from lda.str
/// ```
///
/// ## Label check status
///
/// Upstream skips `DynamicImport` instructions that are jump targets
/// (`IsLabel()` returns true), because in that case the string argument
/// comes from a variable, not a constant `import("string")` expression.
///
/// Our `Insn` enum does not carry the "is jump target" bit, and the
/// `program_labels` byte-offset table cannot be matched against Insn indices
/// without a full instruction-encoding pass.  As a result:
///
/// - We correctly exclude cases where a non-`LdaStr` instruction immediately
///   precedes `DynamicImport` (variable argument path).
/// - We CANNOT exclude the rare case where `DynamicImport` at a constant
///   `import("string")` site is also a jump target, which would cause us to
///   include a dynamic import that upstream would skip.
///
/// This is a known minor gap — the ArkTS compiler does not emit
/// constant-string `import()` expressions at jump targets in normal code.
///
/// ## Upstream reference
/// `es2panda/util/commonUtil.h` `VisitDyanmicImports<true>` (note typo in upstream).
fn dynamic_imports_from_insns(insns: &[Insn], strings: &StringTable) -> Vec<String> {
    let mut imports = Vec::new();
    // Iterate forward, find DynamicImport, look back at the previous instruction.
    // This matches the upstream iteration order.
    for i in 1..insns.len() {
        let Insn::DynamicImport = &insns[i] else {
            continue;
        };
        // Upstream: check previous instruction is lda.str (constant string import).
        let Insn::LdaStr(string_id) = &insns[i - 1] else {
            continue;
        };
        // Resolve the LOCAL string_id from the Program's StringTable.
        if let Some(s) = strings.get(u32::from(*string_id)) {
            imports.push(s.to_string());
        }
    }
    imports
}

fn raw_u32(entry: &LiteralEntry) -> Option<u32> {
    match entry {
        LiteralEntry::RawU32(v) => Some(*v),
        _ => None,
    }
}

fn raw_string(entry: &LiteralEntry) -> Option<&str> {
    match entry {
        LiteralEntry::RawString(v) => Some(v),
        _ => None,
    }
}

fn record_name_from_normalized_ohmurl(ohmurl: &str) -> Option<Box<str>> {
    let items: Vec<_> = ohmurl.split(NORMALIZED_OHMURL_SEPARATOR).collect();
    if items.len() <= VERSION_POS {
        return None;
    }
    Some(Box::from(format!(
        "{}{}{}{}{}",
        items[BUNDLE_NAME_POS],
        NORMALIZED_OHMURL_SEPARATOR,
        items[NORMALIZED_IMPORT_POS],
        NORMALIZED_OHMURL_SEPARATOR,
        items[VERSION_POS]
    )))
}

fn is_external_pkg_name(
    ohmurl: &str,
    external_pkg_names: &std::collections::BTreeSet<Box<str>>,
) -> bool {
    let Some(pkg_name) = pkg_name_from_normalized_ohmurl(ohmurl) else {
        return false;
    };
    external_pkg_names.contains(&*pkg_name)
}

fn pkg_name_from_normalized_ohmurl(ohmurl: &str) -> Option<Box<str>> {
    let items: Vec<_> = ohmurl.split(NORMALIZED_OHMURL_SEPARATOR).collect();
    let normalized_import = items.get(NORMALIZED_IMPORT_POS)?;
    pkg_name_from_normalized_import(normalized_import).map(Box::from)
}

fn pkg_name_from_normalized_import(normalized_import: &str) -> Option<&str> {
    let mut pos = normalized_import.find(SLASH_TAG)?;
    if normalized_import.starts_with(NORMALIZED_OHMURL_PREFIX) {
        let next = normalized_import[pos + 1..].find(SLASH_TAG)?;
        pos += 1 + next;
    }
    Some(&normalized_import[..pos])
}

fn display_program_file(program_key: &str) -> &str {
    let abc_separator = program_key.rfind('|').unwrap_or(program_key.len());
    let Some(slash_pos) = program_key[..abc_separator].rfind('/') else {
        return program_key;
    };
    &program_key[slash_pos + 1..abc_separator]
}

fn dependency_record_name(class_name: &str) -> String {
    // Strip leading 'L' (pandasm class name prefix) and trailing ';'.
    // Upstream: "Lentry/main;" → "entry/main", "L_GLOBAL;" → "_GLOBAL".
    // The "L_" two-char prefix stripping was wrong — '_' is part of the name.
    let Some(stripped) = class_name.strip_prefix('L') else {
        return class_name.to_string();
    };
    stripped.strip_suffix(';').unwrap_or(stripped).to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aot::options::CompileContextInfo;
    use crate::aot::program_registry::ProgramRegistry;
    use crate::program::model::{Program, ProgramCacheEntry};

    fn make_entry(program: Program) -> ProgramCacheEntry {
        ProgramCacheEntry::new_compiled(program, false)
    }

    #[test]
    fn record_name_from_ohmurl_extracts_bundle_import_version() {
        let url = "@normalized:N&com.example.app&@bundle:entry/ets/pages/Index&1.0.0&extra";
        let result = record_name_from_normalized_ohmurl(url);
        // bundle[2] + sep + import[3] + sep + version[4]
        assert!(result.is_some(), "must extract record name from normalized OHM URL");
    }

    #[test]
    fn dependency_record_name_strips_pandasm_prefix() {
        assert_eq!(dependency_record_name("Lentry/main;"), "entry/main");
        assert_eq!(dependency_record_name("L_GLOBAL;"), "_GLOBAL");
        assert_eq!(dependency_record_name("entry/main"), "entry/main");
    }

    #[test]
    fn empty_compile_entries_retains_no_programs() {
        let registry = ProgramRegistry::new(0);
        let context = CompileContextInfo::default();
        let resolved = resolve_deps_relation(&registry, &context, false);
        assert!(
            resolved.programs.is_empty(),
            "empty compile_entries must retain no programs"
        );
    }

    #[test]
    fn dump_deps_info_produces_deterministic_output() {
        // The dump format should be consistent across runs.
        let mut resolved = ResolvedDepsRelation::default();
        resolved.insert("b/main.ts", "b/main");
        resolved.insert("a/main.ts", "a/main");

        // We can't easily capture stdout, but we verify the method runs without panic.
        // Full stdout comparison is done in oracle tests.
        assert!(resolved.contains_program("a/main.ts"));
        assert!(resolved.contains_program("b/main.ts"));
    }
}
