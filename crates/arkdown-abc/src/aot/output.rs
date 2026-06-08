//! AOT output generator.
//!
//! This module implements `AotOutputGenerator`, which converts a `ProgramRegistry`
//! into final output: merged ABC, per-file ABCs, base64 output, or proto output.
//!
//! ## Upstream algorithm (`GenerateProgram`)
//!
//! 1. Dump Program info if requested.
//! 2. For single Program with no output, generate base64.
//! 3. For single Program with `--outputProto` and no global optimization,
//!    write proto and return early.
//! 4. Remove redundant records if `--remove-redundant-file`.
//! 5. Schedule cache emit jobs (write `.protoBin` files for entries that need it).
//! 6. Emit single or merged ABC.
//! 7. Optionally run transform library.
//!
//! ## Upstream reference
//! - `es2panda/aot/main.cpp` — `GenerateProgram`, `GenerateProgramsByWorkers`
//! - `es2panda/aot/emitFiles.cpp` — `EmitFileQueue`, `EmitMergedAbcJob`

use std::io;
use std::path::Path;

use crate::aot::cache_store::ProgramCacheStore;
use crate::aot::options::{AotCompileOptions, AotOutput};
use crate::aot::program_registry::ProgramRegistry;
use crate::cache::ProgramProtoFile;
use crate::emit::file::version_for_target_api;
use crate::error::AbcError;
use crate::program::to_abc::{program_to_abc_file, registry_to_abc_file};
use crate::program_cache::program_for_file;

// ── AotOutputResult ───────────────────────────────────────────────────────────

/// The result of an AOT output generation pass.
#[derive(Debug)]
pub struct AotOutputResult {
    /// Written ABC file path(s).
    pub abc_paths: Vec<std::path::PathBuf>,
    /// Written proto output path (for `--outputProto`).
    pub proto_path: Option<std::path::PathBuf>,
    /// Whether the output contains debug information.
    pub has_debug_info: bool,
    /// Target API version this output was compiled for.
    pub api_version: u32,
    /// Base64-encoded output (for base64 output mode).
    pub base64_output: Option<String>,
}

// ── AotOutputGenerator ────────────────────────────────────────────────────────

/// AOT output generator.
///
/// Converts a finalized `ProgramRegistry` into ABC or proto output,
/// writes cache files for programs that need it, and returns a structured
/// output result.
pub struct AotOutputGenerator<'a> {
    options: &'a AotCompileOptions,
    cache_store: &'a ProgramCacheStore,
}

impl<'a> AotOutputGenerator<'a> {
    pub fn new(options: &'a AotCompileOptions, cache_store: &'a ProgramCacheStore) -> Self {
        Self { options, cache_store }
    }

    /// Generate output from the finalized `ProgramRegistry`.
    ///
    /// Mirrors upstream `GenerateProgram`.
    pub fn generate(&self, registry: &ProgramRegistry) -> Result<AotOutputResult, AbcError> {
        let panda_version = version_for_target_api(
            self.options.target_api_version,
            self.options.target_api_sub_version.as_deref(),
        );

        let has_debug_info = self.options.effective_debug_info();

        // Step 2: single Program with no output → base64.
        if registry.count() == 1 {
            if let AotOutput::Base64 = &self.options.output {
                let base64 = self.emit_base64(registry, panda_version)?;
                return Ok(AotOutputResult {
                    abc_paths: vec![],
                    proto_path: None,
                    has_debug_info,
                    api_version: self.options.target_api_version,
                    base64_output: Some(base64),
                });
            }
        }

        // Step 3: single Program with --outputProto → write proto, return early.
        if registry.count() == 1 {
            if let Some(proto_path) = &self.options.output_proto {
                let proto_result = self.emit_output_proto(registry, proto_path, panda_version)?;
                return Ok(AotOutputResult {
                    abc_paths: vec![],
                    proto_path: Some(proto_result),
                    has_debug_info,
                    api_version: self.options.target_api_version,
                    base64_output: None,
                });
            }
        }

        // Step 5: emit cache files for programs that need it.
        self.emit_cache_jobs(registry, panda_version)?;

        // Step 6: emit merged or per-file ABC.
        let abc_paths = if self.options.merge_abc {
            self.emit_merged_abc(registry, panda_version)?
        } else {
            self.emit_single_abcs(registry, panda_version)?
        };

        Ok(AotOutputResult {
            abc_paths,
            proto_path: None,
            has_debug_info,
            api_version: self.options.target_api_version,
            base64_output: None,
        })
    }

    /// Emit cache files for Programs that have `need_update_cache = true`.
    ///
    /// Mirrors upstream `EmitProgramCacheJob` in `emitFiles.cpp`.
    fn emit_cache_jobs(
        &self,
        registry: &ProgramRegistry,
        panda_version: u32,
    ) -> Result<(), AbcError> {
        for (key, _, entry) in registry.iter_in_key_order() {
            if !entry.need_update_cache {
                continue;
            }
            let Some(cache_path) = self.options.cache_files.get(key) else {
                continue;
            };

            // Build a temporary AbcFile from this Program's records.
            let abc_file = program_to_abc_file(&entry.program)?;

            // Extract the first (or only) record index.
            if abc_file.records.is_empty() {
                continue;
            }

            let program_proto =
                program_for_file(&abc_file, self.options.target_api_version)
                    .map_err(AbcError::Io)?;

            self.cache_store
                .write_source_program(cache_path, entry.hash_code, program_proto)
                .map_err(AbcError::Io)?;
            tracing::debug!(
                key = key,
                cache = %cache_path.display(),
                "wrote ProgramCache file"
            );
        }
        Ok(())
    }

    /// Emit a single merged ABC file containing all Programs' records.
    ///
    /// Upstream: `EmitMergedAbcJob::Run()` in `emitFiles.cpp`.
    fn emit_merged_abc(
        &self,
        registry: &ProgramRegistry,
        panda_version: u32,
    ) -> Result<Vec<std::path::PathBuf>, AbcError> {
        let output_path = match &self.options.output {
            AotOutput::Path(p) => p.clone(),
            AotOutput::Base64 | AotOutput::PerInput => {
                return Err(AbcError::UnsupportedConstruct {
                    construct: "merge_abc=true but output is not a file path".to_string(),
                    file: String::new(),
                })
            }
        };

        let abc_file = registry_to_abc_file(registry)?;
        let bytes = abc_file.serialize_with_version(panda_version);

        write_abc_file(&output_path, &bytes).map_err(AbcError::Io)?;

        Ok(vec![output_path])
    }

    /// Emit per-input ABC files (non-merge mode).
    ///
    /// Upstream: `EmitFileJob::Run()` in `emitFiles.cpp`.
    fn emit_single_abcs(
        &self,
        registry: &ProgramRegistry,
        panda_version: u32,
    ) -> Result<Vec<std::path::PathBuf>, AbcError> {
        let mut output_paths = Vec::new();

        for (key, _, entry) in registry.iter_in_key_order() {
            let output_path = derive_output_path(key, &self.options.output);
            let abc_file = program_to_abc_file(&entry.program)?;
            let bytes = abc_file.serialize_with_version(panda_version);
            write_abc_file(&output_path, &bytes).map_err(AbcError::Io)?;
            output_paths.push(output_path);
        }

        Ok(output_paths)
    }

    /// Emit base64-encoded ABC for single-program mode.
    fn emit_base64(&self, registry: &ProgramRegistry, panda_version: u32) -> Result<String, AbcError> {
        // Upstream: base64 output writes the single program to stdout.
        let abc_file = registry_to_abc_file(registry)?;
        let bytes = abc_file.serialize_with_version(panda_version);
        Ok(base64_encode(&bytes))
    }

    /// Emit proto output for single-program mode with `--outputProto`.
    fn emit_output_proto(
        &self,
        registry: &ProgramRegistry,
        proto_path: &Path,
        panda_version: u32,
    ) -> Result<std::path::PathBuf, AbcError> {
        let abc_file = registry_to_abc_file(registry)?;
        let program_proto =
            program_for_file(&abc_file, self.options.target_api_version)
                .map_err(AbcError::Io)?;

        ProgramProtoFile::write(proto_path, program_proto).map_err(AbcError::Io)?;

        Ok(proto_path.to_path_buf())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Write ABC bytes to a file, creating parent directories as needed.
fn write_abc_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, bytes)
}

/// Derive the output path for a single Program in non-merge mode.
fn derive_output_path(program_key: &str, output: &AotOutput) -> std::path::PathBuf {
    match output {
        AotOutput::Path(p) => p.clone(),
        AotOutput::PerInput | AotOutput::Base64 => {
            // Derive from program key (file name without extension + ".abc").
            let stem = std::path::Path::new(program_key)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            std::path::PathBuf::from(format!("{stem}.abc"))
        }
    }
}

/// Base64 encode bytes using standard alphabet.
fn base64_encode(bytes: &[u8]) -> String {
    const BASE64_CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        output.push(BASE64_CHARS[(n >> 18) as usize] as char);
        output.push(BASE64_CHARS[((n >> 12) & 0x3f) as usize] as char);
        output.push(if chunk.len() > 1 {
            BASE64_CHARS[((n >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            BASE64_CHARS[(n & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aot::cache_store::ProgramCacheStore;
    use crate::aot::options::{AotCompileOptions, AotOutput, CompileContextInfo, SourceFile};
    use crate::aot::program_registry::ProgramRegistry;
    use crate::emit::record::ClassRecord;
    use crate::program::model::{Program, ProgramCacheEntry};
    use oxc_index::IndexVec;
    use rustc_hash::FxHashMap;
    use std::path::PathBuf;

    fn make_options(output: AotOutput) -> AotCompileOptions {
        AotCompileOptions {
            source_files: IndexVec::new(),
            output,
            merge_abc: true,
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
            enable_abc_input: false,
            dump_deps_info: false,
            remove_redundant_file: false,
            output_proto: None,
            module_record_field_name: None,
            enable_annotations: false,
            use_define_semantic: false,
            require_global_optimization: false,
        }
    }

    fn make_minimal_registry() -> ProgramRegistry {
        let mut program = Program::new_ecmascript();
        program.add_record(ClassRecord {
            name: "test/main".to_string(),
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
        });
        let mut registry = ProgramRegistry::new(1);
        registry
            .insert_source_program("test.ts", ProgramCacheEntry::new_compiled(program, false))
            .unwrap();
        registry
    }

    #[test]
    fn merge_abc_output_writes_file_with_panda_magic() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("output.abc");
        let options = make_options(AotOutput::Path(output_path.clone()));
        let cache_store = ProgramCacheStore::new(12, None);
        let generator = AotOutputGenerator::new(&options, &cache_store);
        let registry = make_minimal_registry();

        let result = generator.generate(&registry).unwrap();
        assert_eq!(result.abc_paths.len(), 1);
        assert_eq!(result.abc_paths[0], output_path);

        let bytes = std::fs::read(&output_path).unwrap();
        assert_eq!(&bytes[0..8], b"PANDA\x00\x00\x00");
    }

    #[test]
    fn base64_output_produces_non_empty_string() {
        let options = make_options(AotOutput::Base64);
        let cache_store = ProgramCacheStore::new(12, None);
        let generator = AotOutputGenerator::new(&options, &cache_store);
        let registry = make_minimal_registry();

        let result = generator.generate(&registry).unwrap();
        assert!(
            result.base64_output.is_some(),
            "base64 mode must produce output"
        );
        let b64 = result.base64_output.unwrap();
        assert!(!b64.is_empty(), "base64 output must be non-empty");
    }

    #[test]
    fn output_proto_writes_proto_file_and_returns_early() {
        let dir = tempfile::tempdir().unwrap();
        let proto_path = dir.path().join("output.protoBin");
        let mut options = make_options(AotOutput::Path(dir.path().join("output.abc")));
        options.output_proto = Some(proto_path.clone());

        let cache_store = ProgramCacheStore::new(12, None);
        let generator = AotOutputGenerator::new(&options, &cache_store);
        let registry = make_minimal_registry();

        let result = generator.generate(&registry).unwrap();
        assert!(
            result.proto_path.is_some(),
            "--outputProto must produce a proto output"
        );
        assert_eq!(result.abc_paths.len(), 0, "--outputProto must return before ABC emission");
    }
}
