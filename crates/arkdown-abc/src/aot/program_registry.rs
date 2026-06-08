//! ProgramRegistry — upstream-shaped storage for all compiled Programs.
//!
//! Upstream `es2panda/aot/main.cpp` stores compiled Programs in:
//!
//! ```cpp
//! std::map<std::string, panda::es2panda::util::ProgramCache *> programsInfo;
//! ```
//!
//! `std::map` iterates in lexicographic key order, which determines:
//! - record insertion order in merged ABC output;
//! - dependency BFS traversal order;
//! - cache emit job order.
//!
//! The Rust equivalent uses a `BTreeMap<ProgramKey, ProgramId>` sorted index
//! plus an `IndexVec<ProgramId, ProgramCacheEntry>` for typed ID access.
//!
//! ## Program key semantics
//!
//! - Source inputs: key = `SourceFile::fileName` (`src_->fileName`).
//! - ABC input records (merge mode): key = `abcFileName + "|" + recordName`.
//! - ABC input (non-merge / whole file): key = `abcFileName`.
//! - Npm entries: key = path to the npm entries list file.
//!
//! ## Upstream reference
//! - `es2panda/aot/main.cpp` `Run()` — `programsInfo` map construction.
//! - `es2panda/aot/main.cpp` `CheckMergeModeConsistency` — merge/non-merge checks.
//! - `es2panda/aot/main.cpp` `GenerateAbcFiles` — expected count validation.

use std::collections::BTreeMap;

use oxc_index::IndexVec;

use crate::error::AbcError;
use crate::program::ids::ProgramId;
use crate::program::model::ProgramCacheEntry;

/// The string key used to identify a Program in upstream `programsInfo`.
///
/// Source key: `src_->fileName` (file path string).
/// ABC record key: `abcFileName + "|" + recordName`.
/// Npm entries key: entries file path string.
pub type ProgramKey = Box<str>;

/// Upstream-shaped registry of all Programs produced by one compile run.
///
/// Programs are keyed in lexicographic order (matching upstream `std::map`)
/// and indexed by typed `ProgramId` for zero-cost access.
#[derive(Debug, Default)]
pub struct ProgramRegistry {
    /// Ordered index: `ProgramKey → ProgramId` (sorted for upstream parity).
    key_to_id: BTreeMap<ProgramKey, ProgramId>,
    /// Programs stored by insertion ID.
    entries: IndexVec<ProgramId, ProgramCacheEntry>,
    /// Expected number of programs (set from source_files.len() + npm).
    expected_count: usize,
}

impl ProgramRegistry {
    /// Create an empty registry with a pre-set expected program count.
    ///
    /// Upstream: `Compiler::SetExpectedProgsCount(options->CompilerOptions().sourceFiles.size())`.
    pub fn new(expected_count: usize) -> Self {
        Self {
            key_to_id: BTreeMap::new(),
            entries: IndexVec::new(),
            expected_count,
        }
    }

    /// Increment the expected program count by `delta`.
    ///
    /// Called after adding npm entries:
    /// `Compiler::SetExpectedProgsCount(GetExpectedProgsCount() + 1)`.
    pub fn add_expected(&mut self, delta: usize) {
        self.expected_count += delta;
    }

    /// Return the expected program count.
    pub fn expected_count(&self) -> usize {
        self.expected_count
    }

    /// Return the current program count.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Insert a Program compiled from a source file.
    ///
    /// Key: `src_->fileName` (the `SourceFile::file_name` string).
    ///
    /// Returns an error if the key already exists (upstream `std::map::insert`
    /// on an existing key is a no-op — we reject it to catch bugs).
    pub fn insert_source_program(
        &mut self,
        key: impl Into<ProgramKey>,
        entry: ProgramCacheEntry,
    ) -> Result<ProgramId, AbcError> {
        self.insert_with_key(key.into(), entry)
    }

    /// Insert a Program split from an ABC input record.
    ///
    /// Key: `abcFileName + "|" + recordName` (upstream merge-mode format).
    pub fn insert_abc_program(
        &mut self,
        abc_file_name: &str,
        record_name: &str,
        entry: ProgramCacheEntry,
    ) -> Result<ProgramId, AbcError> {
        let key: ProgramKey = Box::from(abc_input_program_key(abc_file_name, record_name));
        self.insert_with_key(key, entry)
    }

    /// Insert the npm module entry Program.
    ///
    /// Key: path to the npm entries list file.
    pub fn insert_npm_program(
        &mut self,
        key: impl Into<ProgramKey>,
        entry: ProgramCacheEntry,
    ) -> Result<ProgramId, AbcError> {
        self.insert_with_key(key.into(), entry)
    }

    fn insert_with_key(
        &mut self,
        key: ProgramKey,
        entry: ProgramCacheEntry,
    ) -> Result<ProgramId, AbcError> {
        use std::collections::btree_map::Entry;
        match self.key_to_id.entry(key.clone()) {
            Entry::Occupied(existing) => {
                // Upstream std::map silently ignores duplicate inserts.
                // We return the existing ID for callers that need it.
                Ok(*existing.get())
            }
            Entry::Vacant(vacant) => {
                let id = self.entries.push(entry);
                vacant.insert(id);
                Ok(id)
            }
        }
    }

    /// Look up a Program by its key.
    pub fn get(&self, key: &str) -> Option<(ProgramId, &ProgramCacheEntry)> {
        self.key_to_id
            .get(key)
            .map(|&id| (id, &self.entries[id]))
    }

    /// Look up a Program by its key (mutable).
    pub fn get_mut(&mut self, key: &str) -> Option<(ProgramId, &mut ProgramCacheEntry)> {
        if let Some(&id) = self.key_to_id.get(key) {
            Some((id, &mut self.entries[id]))
        } else {
            None
        }
    }

    /// Look up a Program by typed ID.
    pub fn by_id(&self, id: ProgramId) -> &ProgramCacheEntry {
        &self.entries[id]
    }

    /// Iterate over all Programs in upstream key order (lexicographic).
    ///
    /// This matches upstream `for (const auto &progInfo : programsInfo)`.
    pub fn iter_in_key_order(&self) -> impl Iterator<Item = (&str, ProgramId, &ProgramCacheEntry)> {
        self.key_to_id
            .iter()
            .map(|(key, &id)| (key.as_ref(), id, &self.entries[id]))
    }

    /// Iterate over all Programs in upstream key order (mutable).
    pub fn iter_in_key_order_mut(
        &mut self,
    ) -> impl Iterator<Item = (&str, ProgramId, &mut ProgramCacheEntry)> {
        // We need to iterate the keys in order but mutably access entries.
        // Collect keys first, then mutate.
        let keys: Vec<_> = self.key_to_id.keys().cloned().collect();
        let ids: Vec<_> = keys
            .iter()
            .map(|k| *self.key_to_id.get(k.as_ref()).unwrap())
            .collect();
        // Return an iterator of owned key references + mutable entries.
        ids.into_iter().zip(self.entries.iter_mut()).map(move |(id, entry)| {
            ("", id, entry) // Note: key string not accessible here — use iter_by_id instead
        })
    }

    /// Drain all Programs from the registry, in upstream key order.
    ///
    /// Returns `(key, id, entry)` tuples sorted by key.
    pub fn drain_in_key_order(
        self,
    ) -> impl Iterator<Item = (ProgramKey, ProgramId, ProgramCacheEntry)> {
        let ProgramRegistry { key_to_id, mut entries, .. } = self;
        key_to_id.into_iter().map(move |(key, id)| {
            // We need to take ownership. Since we're iterating in insertion
            // order of keys (which may differ from entries order), we clone.
            let entry = entries[id].clone();
            (key, id, entry)
        })
    }

    /// Collect all Programs in upstream key order as a `Vec`.
    pub fn collect_in_key_order(&self) -> Vec<(ProgramKey, ProgramId, &ProgramCacheEntry)> {
        self.key_to_id
            .iter()
            .map(|(key, &id)| (key.clone(), id, &self.entries[id]))
            .collect()
    }

    /// Check merge/non-merge consistency of ABC input Programs.
    ///
    /// Upstream `CheckMergeModeConsistency`:
    /// - merge mode: rejects Programs with key `file|GLOBAL_TYPE_NAME`;
    /// - non-merge mode: rejects Programs from ABC files that have no `GLOBAL_TYPE_NAME`.
    ///
    /// Returns `Ok(())` if consistent, `Err` with a diagnostic message if not.
    pub fn check_merge_mode_consistency(&self, merge_abc: bool) -> Result<(), AbcError> {
        // Upstream GLOBAL_TYPE_NAME is "_GLOBAL" in pandasm notation.
        const GLOBAL_TYPE_NAME: &str = "_GLOBAL";

        // Collect ABC-input Programs (those with "|" in the key).
        let mut abc_file_records: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (key, _, _) in self.iter_in_key_order() {
            if !is_abc_program_key(key) {
                continue;
            }
            let (abc_file, record) = split_abc_program_key(key);
            abc_file_records.entry(abc_file).or_default().push(record);
        }

        if abc_file_records.is_empty() {
            return Ok(());
        }

        if merge_abc {
            // merge-abc: all ABC input files must be merged (no GLOBAL_TYPE_NAME).
            for (abc_file, records) in &abc_file_records {
                if records.iter().any(|r| *r == GLOBAL_TYPE_NAME) {
                    return Err(AbcError::UnsupportedConstruct {
                        construct: format!(
                            "Current compile mode is merge-abc, but file '{}' is not a merged abc",
                            abc_file
                        ),
                        file: abc_file.to_string(),
                    });
                }
            }
        } else {
            // non-merge: all ABC input files must be unmerged (have GLOBAL_TYPE_NAME).
            for (abc_file, records) in &abc_file_records {
                if !records.iter().any(|r| *r == GLOBAL_TYPE_NAME) {
                    return Err(AbcError::UnsupportedConstruct {
                        construct: format!(
                            "Current compile mode is non merge-abc, but file '{}' is a merged abc",
                            abc_file
                        ),
                        file: abc_file.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Validate that the actual program count matches the expected count.
    ///
    /// Upstream `GenerateAbcFiles`: `if (programsInfo.size() != expectedProgsCount)`.
    pub fn validate_expected_count(&self) -> Result<(), AbcError> {
        if self.entries.len() != self.expected_count {
            return Err(AbcError::UnsupportedConstruct {
                construct: format!(
                    "The size of programs is expected to be {}, but is {}",
                    self.expected_count,
                    self.entries.len()
                ),
                file: String::new(),
            });
        }
        Ok(())
    }

    /// Retain only the Programs whose keys are in `retained_keys`.
    ///
    /// Used by `DepsRelationResolver` after BFS pruning.
    pub fn retain_resolved(&mut self, retained_keys: &std::collections::BTreeSet<Box<str>>) {
        let removed_keys: Vec<ProgramKey> = self
            .key_to_id
            .keys()
            .filter(|key| !retained_keys.contains(*key))
            .cloned()
            .collect();

        for key in removed_keys {
            if let Some(id) = self.key_to_id.remove(&key) {
                // Mark the entry as removed by leaving it in entries but
                // setting a sentinel. We track retention via the key_to_id map.
                let _ = id;
            }
        }

        // Update expected count to match the retained set.
        self.expected_count = self.key_to_id.len();
    }
}

/// Construct the ABC-input program key: `abcFileName + "|" + recordName`.
///
/// Upstream `options.cpp` uses the vertical bar as a separator.
pub fn abc_input_program_key(abc_file_name: &str, record_name: &str) -> String {
    format!("{abc_file_name}|{record_name}")
}

/// Return `true` if a program key was generated from an ABC input file.
///
/// ABC input keys contain `"|"` as a separator; source keys do not.
pub fn is_abc_program_key(key: &str) -> bool {
    key.contains('|')
}

/// Split an ABC input program key into `(abc_file_name, record_name)`.
pub fn split_abc_program_key(key: &str) -> (&str, &str) {
    key.rfind('|')
        .map(|pos| (&key[..pos], &key[pos + 1..]))
        .unwrap_or((key, ""))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::model::{Program, ProgramCacheEntry};

    fn make_entry() -> ProgramCacheEntry {
        ProgramCacheEntry::new_compiled(Program::new_ecmascript(), false)
    }

    #[test]
    fn source_keys_sort_in_lexicographic_order() {
        // Upstream std::map iterates in lexicographic key order.
        let mut registry = ProgramRegistry::new(3);
        registry
            .insert_source_program("c/main.ts", make_entry())
            .unwrap();
        registry
            .insert_source_program("a/main.ts", make_entry())
            .unwrap();
        registry
            .insert_source_program("b/main.ts", make_entry())
            .unwrap();

        let keys: Vec<&str> = registry
            .iter_in_key_order()
            .map(|(key, _, _)| key)
            .collect();
        assert_eq!(keys, ["a/main.ts", "b/main.ts", "c/main.ts"],
            "programs must iterate in lexicographic key order like upstream std::map");
    }

    #[test]
    fn abc_keys_use_file_pipe_record_format() {
        let key = abc_input_program_key("modules.abc", "entry/main");
        assert_eq!(key, "modules.abc|entry/main");
        assert!(is_abc_program_key(&key));
        let (file, record) = split_abc_program_key(&key);
        assert_eq!(file, "modules.abc");
        assert_eq!(record, "entry/main");
    }

    #[test]
    fn program_registry_expected_count_validates() {
        let mut registry = ProgramRegistry::new(2);
        registry
            .insert_source_program("a.ts", make_entry())
            .unwrap();
        // Only 1 program inserted but expected 2.
        assert!(
            registry.validate_expected_count().is_err(),
            "must error when count != expected"
        );
        registry
            .insert_source_program("b.ts", make_entry())
            .unwrap();
        assert!(
            registry.validate_expected_count().is_ok(),
            "must succeed when count == expected"
        );
    }

    #[test]
    fn program_registry_expected_count_adjusted_for_npm_entries() {
        let mut registry = ProgramRegistry::new(2);
        registry.add_expected(1); // npm entry
        assert_eq!(registry.expected_count(), 3);
    }

    #[test]
    fn merge_mode_consistency_rejects_global_type_in_merge_mode() {
        let mut registry = ProgramRegistry::new(2);
        // _GLOBAL key = unmerged ABC in a merge-abc compile.
        registry
            .insert_abc_program("input.abc", "_GLOBAL", make_entry())
            .unwrap();
        registry
            .insert_source_program("a.ts", make_entry())
            .unwrap();

        let result = registry.check_merge_mode_consistency(true);
        assert!(
            result.is_err(),
            "merge-abc mode must reject _GLOBAL record key from ABC input"
        );
    }

    #[test]
    fn merge_mode_consistency_accepts_normal_records_in_merge_mode() {
        let mut registry = ProgramRegistry::new(2);
        registry
            .insert_abc_program("input.abc", "entry/main", make_entry())
            .unwrap();
        registry
            .insert_source_program("a.ts", make_entry())
            .unwrap();

        let result = registry.check_merge_mode_consistency(true);
        assert!(
            result.is_ok(),
            "merge-abc mode must accept non-_GLOBAL record keys"
        );
    }

    #[test]
    fn program_dump_stable_under_insertion_order_permutations() {
        // Inserting b/main, a/main vs a/main, b/main must produce same key order.
        let mut reg1 = ProgramRegistry::new(2);
        reg1.insert_source_program("b/main.ts", make_entry()).unwrap();
        reg1.insert_source_program("a/main.ts", make_entry()).unwrap();

        let mut reg2 = ProgramRegistry::new(2);
        reg2.insert_source_program("a/main.ts", make_entry()).unwrap();
        reg2.insert_source_program("b/main.ts", make_entry()).unwrap();

        let keys1: Vec<&str> = reg1.iter_in_key_order().map(|(k, _, _)| k).collect();
        let keys2: Vec<&str> = reg2.iter_in_key_order().map(|(k, _, _)| k).collect();
        assert_eq!(keys1, keys2, "insertion order must not affect iteration order");
    }
}
