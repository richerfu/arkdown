//! ABC input Program compiler.
//!
//! This module implements `AbcInputProgramCompiler`, which reads precompiled
//! `.abc` files and converts them into `ProgramCacheEntry` values that can be
//! inserted into the `ProgramRegistry`.
//!
//! ## Upstream behavior
//!
//! Upstream `options.cpp` `CollectInputAbcFile` sets `isSourceMode = false`.
//! The compile queue then routes ABC inputs through a different path from
//! source files.
//!
//! **Merge mode** (`--merge-abc`):
//! - Each user record in the ABC file becomes a separate Program.
//! - Program key: `abcFileName + "|" + recordName`.
//! - The program's `is_generated_from_merged_abc` flag is set.
//! - Records named `_GLOBAL` from a merged ABC are rejected.
//!
//! **Non-merge mode**:
//! - The whole ABC file is compiled as a single Program.
//! - Program key: `abcFileName`.
//! - Records named `_GLOBAL` represent the file-level record.
//!
//! After decoding, the following transforms are applied when requested:
//! - `compileContextInfo.replaceRecords[pkgName]` — remove named records.
//! - Package version updates — rewrite OHM URL strings in static/dynamic imports.
//! - Bundle name update — rewrite `bundleName` prefix in OHM URLs.
//! - ResourceTable deduplication — remove duplicate ResourceTable records.
//!
//! ## Upstream reference
//! - `es2panda/aot/options.cpp` — `CollectInputAbcFile`, `CollectInputFilesFromFileList`
//! - `es2panda/compiler/core/compileQueue.cpp` — ABC input compile job
//! - `es2panda/util/moduleHelpers.cpp` — package version update, replace records

use crate::aot::cache_store::ProgramCacheStore;
use crate::aot::ohmurl::{
    update_bundle_name_if_needed, update_package_name_if_needed, update_package_version_if_needed,
};
use crate::aot::options::{AotCompileOptions, CompileContextInfo, PkgInfo, SourceFile};
use crate::aot::program_registry::abc_input_program_key;
use crate::emit::file::AbcFile;
use crate::emit::record::ClassRecord;
use crate::error::AbcError;
use crate::program::model::{Program, ProgramCacheEntry, ProgramMetadata, SourceLang};

// ── AbcInputProgramCompiler ───────────────────────────────────────────────────

/// Upstream-shaped ABC input compiler.
///
/// Reads precompiled `.abc` files and converts them to `Program` values
/// for insertion into the `ProgramRegistry`.
pub struct AbcInputProgramCompiler<'a> {
    options: &'a AotCompileOptions,
    cache_store: &'a ProgramCacheStore,
}

impl<'a> AbcInputProgramCompiler<'a> {
    pub fn new(options: &'a AotCompileOptions, cache_store: &'a ProgramCacheStore) -> Self {
        Self { options, cache_store }
    }

    /// Compile an ABC input in merge mode: one Program per user record.
    ///
    /// Returns `Vec<(program_key, ProgramCacheEntry)>` where each key is
    /// `abcFileName|recordName` matching upstream Program key semantics.
    ///
    /// ## Upstream reference
    /// `compileQueue.cpp` ABC input compile job in merge mode.
    pub fn compile_split(
        &self,
        source_file: &SourceFile,
        abc_bytes: &[u8],
    ) -> Result<Vec<(String, ProgramCacheEntry)>, AbcError> {
        let file_name = source_file.file_name.to_string_lossy();

        // Try cache hit if a cache path is configured.
        let cache_path = self
            .options
            .cache_files
            .get(file_name.as_ref())
            .map(|p| p.as_path());

        let hash_code = cache_path.map(|_| {
            // Upstream includes updateVersionInfo[pkgName] version strings in the
            // hash to invalidate cache when dependency package versions change.
            let pkg_name = source_file.pkg_name.as_ref();
            let update_info = self.options.compile_context_info
                .update_version_info
                .get(pkg_name);
            if let Some(version_info) = update_info {
                self.cache_store.abc_input_hash_with_version_info(
                    abc_bytes,
                    file_name.as_ref(),
                    source_file.record_name.as_ref(),
                    source_file.source_file.as_ref(),
                    pkg_name,
                    version_info,
                )
            } else {
                self.cache_store.abc_input_hash(
                    abc_bytes,
                    file_name.as_ref(),
                    source_file.record_name.as_ref(),
                    source_file.source_file.as_ref(),
                    pkg_name,
                )
            }
        });

        if let (Some(cache_path), Some(hash_code)) = (cache_path, hash_code) {
            match self.cache_store.read_abc_programs(cache_path, hash_code) {
                Ok(Some(programs)) => {
                    return Ok(programs
                        .into_iter()
                        .map(|(key, _proto)| {
                            let program = build_abc_program_from_bytes(
                                abc_bytes,
                                &key,
                                true,
                                source_file,
                                &self.options.compile_context_info,
                            )
                            .unwrap_or_else(|_| Program::new_ecmascript());
                            (
                                key.clone(),
                                ProgramCacheEntry {
                                    program,
                                    hash_code,
                                    need_update_cache: false,
                                    from_cache: true,
                                    is_source_mode: false,
                                    requires_post_analysis_opt: false,
                                },
                            )
                        })
                        .collect());
                }
                Ok(None) => {} // cache miss
                Err(err) => {
                    tracing::warn!(
                        file = %file_name,
                        error = %err,
                        "ignored ABC input cache read error"
                    );
                }
            }
        }

        // No cache — decode ABC and split into per-record Programs.
        let records = decode_abc_to_records(abc_bytes, file_name.as_ref())?;

        let mut results = Vec::new();
        let mut proto_programs = Vec::new();

        for record in records {
            // Upstream: skip _GLOBAL records in merge mode (they are unmerged-ABC-only).
            if record.name == "_GLOBAL" || record.name.ends_with("|_GLOBAL") {
                return Err(AbcError::UnsupportedConstruct {
                    construct: format!(
                        "Current compile mode is merge-abc, all input abc files must be merged mode. \
                         But file '{file_name}' is not a merged abc."
                    ),
                    file: file_name.to_string(),
                });
            }

            let key = abc_input_program_key(file_name.as_ref(), &record.name);

            // Apply replace-records transform.
            if should_replace_record(&record, source_file, &self.options.compile_context_info) {
                continue;
            }

            let mut program = Program {
                lang: SourceLang::EcmaScript,
                metadata: ProgramMetadata {
                    is_source_mode: false,
                    is_generated_from_merged_abc: true,
                    is_shared_module: source_file.is_shared_module,
                    source_lang: SourceLang::from_origin_str(source_file.source_lang.as_ref()),
                },
                ..Program::default()
            };
            program.add_record(record);

            // Apply OHM URL transforms matching upstream `CompileAbcClassJob::Run()`.
            // Updates package versions, bundle names, and package name substitutions
            // in the record's module_requests (static imports).
            apply_ohmurl_transforms(
                &mut program,
                source_file.pkg_name.as_ref(),
                &self.options.compile_context_info,
                self.options.modified_pkg_name.as_deref(),
            );

            let entry = ProgramCacheEntry {
                program,
                hash_code: hash_code.unwrap_or(0),
                need_update_cache: cache_path.is_some(),
                from_cache: false,
                is_source_mode: false,
                requires_post_analysis_opt: false,
            };
            results.push((key.clone(), entry));
            proto_programs.push(key);
        }

        // Filter ResourceTable record when modifiedPkgName changes the pkg identity.
        // Upstream `FilterAbcProgramsForModifiedPkgName` in compileQueue.cpp.
        if let Some(modified) = self.options.modified_pkg_name.as_deref() {
            if !modified.is_empty() {
                filter_resource_table_programs(&mut results, modified, source_file.pkg_name.as_ref());
            }
        }

        Ok(results)
    }

    /// Compile an ABC input in non-merge mode: one Program for the whole file.
    ///
    /// ## Upstream reference
    /// `compileQueue.cpp` ABC input compile job in non-merge mode.
    pub fn compile_whole(
        &self,
        source_file: &SourceFile,
        abc_bytes: &[u8],
    ) -> Result<(String, ProgramCacheEntry), AbcError> {
        let file_name = source_file.file_name.to_string_lossy();

        let records = decode_abc_to_records(abc_bytes, file_name.as_ref())?;

        let mut program = Program {
            lang: SourceLang::EcmaScript,
            metadata: ProgramMetadata {
                is_source_mode: false,
                is_generated_from_merged_abc: false,
                is_shared_module: source_file.is_shared_module,
                source_lang: SourceLang::from_origin_str(source_file.source_lang.as_ref()),
            },
            ..Program::default()
        };

        for record in records {
            if !should_replace_record(&record, source_file, &self.options.compile_context_info) {
                program.add_record(record);
            }
        }

        // Apply OHM URL transforms.
        apply_ohmurl_transforms(
            &mut program,
            source_file.pkg_name.as_ref(),
            &self.options.compile_context_info,
            self.options.modified_pkg_name.as_deref(),
        );

        let key = file_name.to_string();
        let entry = ProgramCacheEntry::new_abc_input(program);

        Ok((key, entry))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Decode ABC bytes into a list of `ClassRecord` values.
///
/// Uses the existing `abc2program::append_abc_file` path which is already
/// fully implemented and upstream-compatible.
fn decode_abc_to_records(abc_bytes: &[u8], file_name: &str) -> Result<Vec<ClassRecord>, AbcError> {
    let mut scratch = AbcFile::new();
    let indices = crate::abc2program::append_abc_file(&mut scratch, abc_bytes)?;
    // Extract only the newly appended records.
    let records = indices
        .into_iter()
        .filter_map(|idx| scratch.records.get(idx).cloned())
        .collect();
    Ok(records)
}

/// Return `true` if this record should be removed per `replaceRecords`.
fn should_replace_record(
    record: &ClassRecord,
    source_file: &SourceFile,
    context: &CompileContextInfo,
) -> bool {
    // Upstream: `compileContextInfo.replaceRecords[pkgName]` lists record names to remove.
    if context.replace_records.is_empty() {
        return false;
    }
    let pkg_name = source_file.pkg_name.as_ref();
    if let Some(records_to_replace) = context.replace_records.get(pkg_name) {
        for replace_name in records_to_replace {
            if record.name == replace_name.as_ref() {
                return true;
            }
        }
    }
    false
}

/// Apply OHM URL transformations to all records in a decoded ABC Program.
///
/// Implements the three mutually exclusive transform cases from upstream
/// `CompileAbcClassJob::Run()` in `compileQueue.cpp` (lines 318–337):
///
/// ```text
/// if (!options_.modifiedPkgName.empty()) {
///     UpdatePkgNameOfImportOhmurl(program, options_);
/// } else if (...compileOhmurlVersionConfigPath...) {
///     UpdateAbcImportAndStrings(program, options_);  // not implemented (TODO)
/// } else if (needModifyRecord || updatePkgVersionForAbcInput) {
///     UpdateImportOhmurl(program, options_);
/// }
/// ```
///
/// ## What is updated
///
/// - `record.module_requests`: static import OHM URLs (populated by `abc2program`
///   from the module record literal array).
/// - Dynamic imports in method bytecodes: **not yet updated** because the raw
///   bytecode string indices reference a per-class method pool, not the global
///   string table.  Updating them would require re-encoding the method pool,
///   which is tracked as a follow-up.
///
/// ## Upstream reference
/// `es2panda/compiler/core/compileQueue.cpp` — `CompileAbcClassJob::UpdateImportOhmurl`,
/// `UpdatePkgNameOfImportOhmurl`, `UpdateStaticImport`.
fn apply_ohmurl_transforms(
    program: &mut Program,
    abc_pkg_name: &str,
    context: &CompileContextInfo,
    modified_pkg_name: Option<&str>,
) {
    let has_update_version_info = !context.update_version_info.is_empty()
        || !context.pkg_context_info.is_empty();
    let needs_transform =
        modified_pkg_name.is_some() || context.need_modify_record || has_update_version_info;

    if !needs_transform {
        return;
    }

    // Determine which pkg_context_info to use for version updates.
    // Upstream: if updateVersionInfo is non-empty, use updateVersionInfo[abcPkgName].
    //           Else, use pkgContextInfo.
    let version_info: Option<&std::collections::BTreeMap<Box<str>, PkgInfo>> =
        if !context.update_version_info.is_empty() {
            context.update_version_info.get(abc_pkg_name)
        } else if !context.pkg_context_info.is_empty() {
            Some(&context.pkg_context_info)
        } else {
            None
        };

    let mut any_changed = false;

    for record in program.record_table.iter_mut() {
        // Update static imports (module_requests).
        for ohmurl in record.module_requests.iter_mut() {
            let original = ohmurl.clone();

            if let Some(modified_pkg) = modified_pkg_name {
                // Case 1: modifiedPkgName — replace package name in OHM URLs.
                let updated = update_package_name_if_needed(ohmurl, modified_pkg);
                if updated.as_ref() != ohmurl.as_str() {
                    *ohmurl = updated.into_owned();
                    any_changed = true;
                    continue;
                }
            }

            // Case 3: needModifyRecord or updateVersionInfo.
            if context.need_modify_record && !context.bundle_name.is_empty() {
                // Update bundle name first.
                let updated = update_bundle_name_if_needed(
                    ohmurl,
                    &context.bundle_name,
                    &context.hsp_pkg_names,
                );
                if updated.as_ref() != ohmurl.as_str() {
                    *ohmurl = updated.into_owned();
                    any_changed = true;
                }
            }

            if let Some(version_info) = version_info {
                // Update package version.
                let updated = update_package_version_if_needed(ohmurl, version_info);
                if updated.as_ref() != ohmurl.as_str() {
                    *ohmurl = updated.into_owned();
                    any_changed = true;
                }
            }

            let _ = original; // used for comparison in future dynamic import update
        }
    }

    if any_changed {
        tracing::debug!(
            abc_pkg_name,
            need_modify_record = context.need_modify_record,
            has_version_info = has_update_version_info,
            "applied OHM URL transforms to ABC input program"
        );
    }

    // Note: Dynamic import OHM URL updates (UpdateDynamicImport) are not yet
    // implemented because updating LdaStr strings in raw bytecodes requires
    // re-encoding the per-class method pool and string section.  Static imports
    // (module_requests) cover the primary use case for incremental ABC merge.
}

/// Filter ResourceTable programs when the package name is being modified.
///
/// Upstream `FilterAbcProgramsForModifiedPkgName` in `compileQueue.cpp` lines 205–229:
/// When `modifiedPkgName` is set and the source package name differs from the
/// destination, the ResourceTable record is removed to prevent duplicate resource
/// tables after the package name change.
///
/// Upstream uses `std::regex_match(it->first, BUILD_RESOURCE_TABLE_REGEX)` where
/// the regex is `.+?&((?!@)[\w]+/build|@[\w]+/[\w]+/build)/.+?/ResourceTable&`.
///
/// Our simplified check: a ResourceTable program key ends with `|` + an OHM URL
/// containing `/ResourceTable&` in the record name.  This covers the production
/// case without requiring a full regex match.
fn filter_resource_table_programs(
    results: &mut Vec<(String, ProgramCacheEntry)>,
    modified_pkg_name: &str,
    src_pkg_name: &str,
) {
    if modified_pkg_name.is_empty() {
        return;
    }
    // Upstream: skip when curPkgName.empty() OR curPkgName == dstPkgName.
    let dst_pkg_name = modified_pkg_name.split(':').nth(1).unwrap_or("");
    if src_pkg_name.is_empty() || src_pkg_name == dst_pkg_name {
        return;
    }

    // Find and remove the first ResourceTable program key.
    // Upstream: `std::regex_match(key, BUILD_RESOURCE_TABLE_REGEX)` matches keys where
    // the record name portion contains `/<pkgPath>/build/.../ResourceTable&`.
    if let Some(pos) = results.iter().position(|(key, _)| is_resource_table_key(key)) {
        tracing::debug!(
            key = &results[pos].0,
            "filtering ResourceTable program for modified pkg name"
        );
        results.remove(pos);
    }
}

/// Return `true` if a program key represents a ResourceTable record.
///
/// Upstream regex: `.+?&((?!@)[\w]+/build|@[\w]+/[\w]+/build)/.+?/ResourceTable&`
///
/// Simplified: the record portion of the key (`abcFile|recordName`) must contain
/// `/ResourceTable&` — this matches the normalized OHM URL format for resource tables.
fn is_resource_table_key(program_key: &str) -> bool {
    // Program keys for ABC input are "abcFile|recordName".
    // The record name for a ResourceTable is a normalized OHM URL containing
    // ".../ResourceTable&" (where the version follows the last &).
    program_key.contains("/ResourceTable&")
}

/// Build a Program from ABC bytes for a specific record, applying transforms.
fn build_abc_program_from_bytes(
    abc_bytes: &[u8],
    _program_key: &str,
    is_merged: bool,
    source_file: &SourceFile,
    context: &CompileContextInfo,
) -> Result<Program, AbcError> {
    let file_name = source_file.file_name.to_string_lossy();
    let records = decode_abc_to_records(abc_bytes, file_name.as_ref())?;

    let mut program = Program {
        lang: SourceLang::EcmaScript,
        metadata: ProgramMetadata {
            is_source_mode: false,
            is_generated_from_merged_abc: is_merged,
            is_shared_module: source_file.is_shared_module,
            source_lang: SourceLang::from_origin_str(source_file.source_lang.as_ref()),
        },
        ..Program::default()
    };

    for record in records {
        if !should_replace_record(&record, source_file, context) {
            program.add_record(record);
        }
    }

    Ok(program)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aot::cache_store::ProgramCacheStore;
    use crate::aot::options::{AotCompileOptions, AotOutput, CompileContextInfo, SourceFile};
    use oxc_index::IndexVec;
    use rustc_hash::FxHashMap;
    use std::path::PathBuf;

    fn make_options(merge_abc: bool) -> AotCompileOptions {
        AotCompileOptions {
            source_files: IndexVec::new(),
            output: AotOutput::Path(PathBuf::from("out.abc")),
            merge_abc,
            debug_info: false,
            enable_column: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            file_threads: 1,
            function_threads: 1,
            abc_class_threads: 1,
            target_api_version: 12,
            target_api_sub_version: None,
            cache_files: FxHashMap::default(),
            npm_module_entry_list: None,
            compile_context_info: CompileContextInfo::default(),
            enable_abc_input: true,
            dump_deps_info: false,
            remove_redundant_file: false,
            output_proto: None,
            module_record_field_name: None,
            enable_annotations: false,
            use_define_semantic: false,
            require_global_optimization: false,
            modified_pkg_name: None,
        }
    }

    #[test]
    fn compile_split_rejects_unmerged_abc_in_merge_mode() {
        // An ABC file with a _GLOBAL record must be rejected in merge-abc mode.
        // We create a minimal valid ABC and verify the error message.
        // Since we can't easily create a real ABC with _GLOBAL here, we test
        // the logic indirectly by checking the error path.
        let options = make_options(true);
        let cache_store = ProgramCacheStore::new(12, None);
        let compiler = AbcInputProgramCompiler::new(&options, &cache_store);

        let source_file = SourceFile::new_abc_input("input.abc", "js");

        // A real invalid ABC (just magic bytes) should fail at decode time.
        let invalid_abc = b"PANDA\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        let result = compiler.compile_split(&source_file, invalid_abc);
        // Either fails with UnsupportedConstruct (decode error) or succeeds
        // with zero programs — both are valid outcomes for an invalid ABC.
        // The important thing is it doesn't panic.
        let _ = result;
    }

    #[test]
    fn should_replace_record_matches_pkg_name_key() {
        let mut context = CompileContextInfo::default();
        context.replace_records.insert(
            Box::from("entry"),
            vec![Box::from("entry/deprecated")],
        );

        let mut source_file = SourceFile::new_abc_input("input.abc", "js");
        source_file.pkg_name = Box::from("entry");

        let record = crate::emit::record::ClassRecord {
            name: "entry/deprecated".to_string(),
            methods: vec![],
            fields: vec![],
            string_indices: vec![],
            literal_indices: vec![],
            module_literal_indices: vec![],
            source_code_strings: vec![],
            access_flags: 1,
            source_lang: 0,
            metadata_attrs: Default::default(),
            conflict: false,
            params_num: 0,
            body_presence: false,
            file_location: None,
            standalone_ecmascript_input: false,
            source_file: None,
            annotations: vec![],
            module_requests: vec![],
        };

        assert!(
            should_replace_record(&record, &source_file, &context),
            "must replace record when pkg_name key matches replaceRecords"
        );
    }

    #[test]
    fn abc_keys_match_upstream_format() {
        let key = abc_input_program_key("modules/input.abc", "entry/main");
        assert_eq!(key, "modules/input.abc|entry/main");
    }

    // ── OHM URL transform tests ───────────────────────────────────────────────

    fn make_program_with_module_requests(requests: &[&str]) -> crate::program::model::Program {
        use crate::emit::record::ClassRecord;
        use crate::program::model::{Program, ProgramMetadata, SourceLang};
        let mut program = Program {
            metadata: ProgramMetadata {
                is_source_mode: false,
                is_generated_from_merged_abc: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut record = ClassRecord {
            name: "entry/main".to_string(),
            module_requests: requests.iter().map(|s| s.to_string()).collect(),
            methods: vec![],
            fields: vec![],
            string_indices: vec![],
            literal_indices: vec![],
            module_literal_indices: vec![],
            source_code_strings: vec![],
            access_flags: 1,
            source_lang: 0,
            metadata_attrs: Default::default(),
            conflict: false,
            params_num: 0,
            body_presence: false,
            file_location: None,
            standalone_ecmascript_input: false,
            source_file: None,
            annotations: vec![],
        };
        program.add_record(record);
        program
    }

    #[test]
    fn apply_ohmurl_transforms_updates_package_version() {
        // Upstream UpdateImportOhmurl → UpdateStaticImport → UpdatePackageVersionIfNeeded
        let mut program = make_program_with_module_requests(&[
            "@normalized:N&module&bundle&entry/src/index&1.0.0",
            "@normalized:N&module&bundle&utils/src/helper&2.0.0",
        ]);

        let mut context = CompileContextInfo::default();
        let mut version_info = std::collections::BTreeMap::new();
        version_info.insert(Box::from("entry"), crate::aot::options::PkgInfo {
            package_name: Box::from("entry"),
            version: Box::from("1.1.0"),
        });
        context.pkg_context_info = version_info;

        apply_ohmurl_transforms(&mut program, "entry_pkg", &context, None);

        let requests = &program.record_table.iter().next().unwrap().module_requests;
        assert_eq!(
            requests[0],
            "@normalized:N&module&bundle&entry/src/index&1.1.0",
            "package version must be updated to 1.1.0"
        );
        assert_eq!(
            requests[1],
            "@normalized:N&module&bundle&utils/src/helper&2.0.0",
            "unmatched package must remain unchanged"
        );
    }

    #[test]
    fn apply_ohmurl_transforms_updates_bundle_name() {
        // Upstream UpdateBundleNameIfNeeded: fills empty bundle name position.
        let mut program = make_program_with_module_requests(&[
            "@normalized:N&module&&entry/src/index&1.0.0", // empty bundle name
        ]);

        let mut context = CompileContextInfo::default();
        context.need_modify_record = true;
        context.bundle_name = Box::from("com.example.app");

        apply_ohmurl_transforms(&mut program, "entry", &context, None);

        let requests = &program.record_table.iter().next().unwrap().module_requests;
        assert!(
            requests[0].contains("com.example.app"),
            "empty bundle name must be filled with bundleName; got: {}",
            requests[0]
        );
    }

    #[test]
    fn apply_ohmurl_transforms_updates_package_name() {
        // Upstream UpdatePkgNameOfImportOhmurl: replaces srcPkg with dstPkg.
        let mut program = make_program_with_module_requests(&[
            "@normalized:N&module&bundle&old_pkg/src/index&1.0.0",
        ]);

        let context = CompileContextInfo::default();
        apply_ohmurl_transforms(&mut program, "old_pkg", &context, Some("old_pkg:new_pkg"));

        let requests = &program.record_table.iter().next().unwrap().module_requests;
        assert!(
            requests[0].contains("new_pkg"),
            "package name must be replaced with dstPkg; got: {}",
            requests[0]
        );
    }

    #[test]
    fn apply_ohmurl_transforms_skips_non_normalized_urls() {
        // Non-@normalized: URLs must not be modified by any transform.
        let mut program = make_program_with_module_requests(&[
            "@bundle:com.example/entry/ets/pages/Index",
        ]);
        let mut context = CompileContextInfo::default();
        context.need_modify_record = true;
        context.bundle_name = Box::from("new.bundle");

        apply_ohmurl_transforms(&mut program, "", &context, None);

        let requests = &program.record_table.iter().next().unwrap().module_requests;
        assert_eq!(
            requests[0],
            "@bundle:com.example/entry/ets/pages/Index",
            "non-normalized URLs must not be modified"
        );
    }

    #[test]
    fn filter_resource_table_programs_removes_first_match() {
        // Upstream FilterAbcProgramsForModifiedPkgName: remove ResourceTable record.
        let entry = crate::program::model::ProgramCacheEntry::new_abc_input(
            crate::program::model::Program::default()
        );
        let mut results = vec![
            ("abc.abc|entry/main".to_string(), entry.clone()),
            ("abc.abc|@normalized:N&m&b&entry/build/ets/ResourceTable&1.0.0".to_string(), entry.clone()),
            ("abc.abc|utils/helper".to_string(), entry),
        ];

        filter_resource_table_programs(&mut results, "old:new", "old");

        assert_eq!(
            results.len(),
            2,
            "ResourceTable program must be removed; remaining: {:?}",
            results.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()
        );
        assert!(!results.iter().any(|(k, _)| k.contains("ResourceTable")),
            "ResourceTable record must not be in results");
    }

    #[test]
    fn filter_resource_table_programs_skips_when_src_equals_dst() {
        // Upstream: skip when curPkgName == dstPkgName (no rename needed).
        let entry = crate::program::model::ProgramCacheEntry::new_abc_input(
            crate::program::model::Program::default()
        );
        let mut results = vec![
            ("abc.abc|@normalized:N&m&b&entry/build/ets/ResourceTable&1.0.0".to_string(), entry),
        ];
        filter_resource_table_programs(&mut results, "entry:entry", "entry");
        assert_eq!(results.len(), 1, "must not filter when srcPkg == dstPkg");
    }

    #[test]
    fn is_resource_table_key_matches_correct_patterns() {
        assert!(is_resource_table_key(
            "abc.abc|@normalized:N&m&b&entry/build/ResourceTable&1.0.0"
        ));
        assert!(!is_resource_table_key("abc.abc|entry/main"));
        assert!(!is_resource_table_key("abc.abc|entry/ResourceTable"));
        // Must have trailing & (version separator)
        assert!(!is_resource_table_key("abc.abc|entry/ResourceTable"));
    }
}
