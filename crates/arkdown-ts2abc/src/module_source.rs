//! Module source classification and collection.
//!
//! `ModuleSourceCollector` classifies source files from a Rolldown bundle
//! output or transform output into typed compile-unit inputs for the ABC
//! stage.
//!
//! The classification distinguishes:
//! - ETS/TS sources that have been transformed to JS (main path)
//! - TS sources that should remain as TS for ABC (bytecode HAR / `TsForAbc`)
//! - JSON module sources (compiled as JSON ABC modules)
//! - Declaration-only inputs (`.d.ts`/`.d.ets` — skipped for ABC)
//! - Lazy-import dynamic entry hints
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `ModuleSourceCollector` | `ark_compiler/transform.js` source pipeline |
//! | `ModuleSourceInputKind` | source classification in `init_config.js` |
//! | `TransformFactInput` | per-module transform output facts |
//! | `JsonModulePlan` | JSON module compile-unit building |
//! | `DeclarationOnlyPlan` | declaration exclusion logic |
//! | `LazyImportPlan` | `process_lazy_import.js` lazy entry analysis |

use std::path::{Path, PathBuf};

use arkdown_core::CompileUnitId;
use arkdown_path::SourceFileName;

use crate::plan::{CompileUnitPlan, ModuleSourceInput, RecordNamePlan};

// ---------------------------------------------------------------------------
// ModuleSourceInputKind
// ---------------------------------------------------------------------------

/// Classification of a module's source for the ABC compilation step.
///
/// # Upstream correspondence
///
/// Corresponds to the per-file classification applied by `init_config.js`
/// and `ark_compiler/transform.js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleSourceInputKind {
    /// Normal JS/TS source (post-ETS-transform) — compiled to ABC record.
    NormalSource,
    /// Preserved TypeScript source for bytecode HAR or `processTs` branches.
    TsForAbc,
    /// JSON module — compiled as a JSON record in ABC.
    JsonModule,
    /// Declaration-only (`.d.ts`/`.d.ets`) — excluded from ABC compilation.
    DeclarationOnly,
    /// Existing `.abc` input for merge mode (`--enable-abc-input`).
    AbcInput,
}

// ---------------------------------------------------------------------------
// ModuleSourceFilePlan
// ---------------------------------------------------------------------------

/// Planned source file input for a single module in the ABC stage.
#[derive(Debug, Clone)]
pub struct ModuleSourceFilePlan {
    /// Source classification.
    pub kind: ModuleSourceInputKind,
    /// Path to the source file on disk.
    pub path: PathBuf,
    /// ABC record name for this module.
    pub record_name: String,
    /// Package name (for `filesInfo.txt` column 5).
    pub package_name: Option<String>,
    /// Whether this is a CommonJS module.
    pub is_commonjs: bool,
    /// Whether this is a shared HSP module.
    pub is_shared_module: bool,
    /// Original source language tag.
    pub origin_source_lang: String,
    /// Source-file attribution string (for debug info / source maps).
    pub source_file: Option<String>,
    /// Per-input output path (non-merge mode).
    pub output_path: Option<PathBuf>,
    /// ProgramCache `.proto` cache file.
    pub cache_file: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// TransformFactInput
// ---------------------------------------------------------------------------

/// Compile metadata produced by the build pipeline for one transformed module.
///
/// `TransformFactInput` carries the transformed output metadata that the ABC
/// stage needs to classify and compile the transformed source.
///
/// # Upstream correspondence
///
/// Corresponds to the module metadata consumed by the ts2abc plan builder.
#[derive(Debug, Clone)]
pub struct TransformFactInput {
    /// The module ID (OHM URL or absolute path).
    pub module_id: String,
    /// Path to the transformed source file (JS or TS).
    pub output_path: PathBuf,
    /// Whether this is a `TsForAbc` output (bytecode HAR or processTs).
    pub is_ts_for_abc: bool,
    /// Whether this is a declaration-only output (skip ABC).
    pub is_declaration_only: bool,
    /// Package name.
    pub package_name: Option<String>,
    /// Source file attribution string.
    pub source_file: Option<String>,
}

// ---------------------------------------------------------------------------
// JsonModulePlan
// ---------------------------------------------------------------------------

/// Plan for a JSON module to be compiled as an ABC record.
///
/// JSON modules are imported with `import config from './config.json'` and
/// compiled to a special JSON ABC record.
///
/// # Upstream correspondence
///
/// Corresponds to `ark_compiler/transform.js` JSON module handling.
#[derive(Debug, Clone)]
pub struct JsonModulePlan {
    /// Path to the JSON file.
    pub path: PathBuf,
    /// The ABC record name for this JSON module.
    pub record_name: String,
    /// Package name.
    pub package_name: Option<String>,
}

// ---------------------------------------------------------------------------
// DeclarationOnlyPlan
// ---------------------------------------------------------------------------

/// A declaration-only entry that is EXCLUDED from ABC compilation.
///
/// Declaration files (`.d.ts`, `.d.ets`) provide type information but do not
/// produce bytecode.  `DeclarationOnlyPlan` records them for completeness but
/// they must NOT be passed to the ABC compiler.
///
/// # Upstream correspondence
///
/// Corresponds to the `declarationEntry` filter in `init_config.js` and
/// `filterCrossplatform` in hvigor.
#[derive(Debug, Clone)]
pub struct DeclarationOnlyPlan {
    /// Path to the declaration file.
    pub path: PathBuf,
    /// Module ID (for source inventory tracking).
    pub module_id: String,
}

// ---------------------------------------------------------------------------
// LazyImportPlan
// ---------------------------------------------------------------------------

/// A dynamic import entry tracked for lazy-chunk dependency analysis.
///
/// Dynamic imports (`import('...')`) become lazy chunks.  The ts2abc stage
/// must record these so that `DepsRelationResolver` can assign them to the
/// correct dependency wave.
///
/// # Upstream correspondence
///
/// Corresponds to `process_lazy_import.js` and the lazy import facts
/// consumed by the SDK es2abc compile-context handoff.
#[derive(Debug, Clone)]
pub struct LazyImportPlan {
    /// The module ID of the lazily-imported module.
    pub module_id: String,
    /// Path to the lazy chunk's source file.
    pub chunk_path: PathBuf,
    /// The record name of the lazy chunk.
    pub record_name: String,
}

// ---------------------------------------------------------------------------
// ModuleSourceCollector
// ---------------------------------------------------------------------------

/// Classifies and collects module source inputs for the ABC compilation step.
///
/// `ModuleSourceCollector` processes the output of the Rolldown bundle stage
/// and the ETS transform stage, and produces the set of [`CompileUnitPlan`]
/// entries for the `Ts2abcPlan`.
///
/// # Upstream correspondence
///
/// Corresponds to `ark_compiler/transform.js` source processing and
/// `rollup-plugin-gen-abc.js::generateBundle` module collection.
pub struct ModuleSourceCollector {
    units: Vec<ModuleSourceFilePlan>,
    json_modules: Vec<JsonModulePlan>,
    declaration_only: Vec<DeclarationOnlyPlan>,
    lazy_imports: Vec<LazyImportPlan>,
    next_id: u32,
}

impl ModuleSourceCollector {
    /// Create a new, empty collector.
    pub fn new() -> Self {
        Self {
            units: Vec::new(),
            json_modules: Vec::new(),
            declaration_only: Vec::new(),
            lazy_imports: Vec::new(),
            next_id: 0,
        }
    }

    /// Collect module sources from a Rolldown bundle output.
    ///
    /// Each chunk in the bundle output produces a `ModuleSourceFilePlan`.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `rollup-plugin-gen-abc.js::generateBundle` module
    /// source collection.
    pub fn collect_from_bundle(
        &mut self,
        chunk_paths: impl IntoIterator<Item = (String, PathBuf)>,
    ) {
        for (module_id, path) in chunk_paths {
            self.add_source(ModuleSourceFilePlan {
                kind: ModuleSourceInputKind::NormalSource,
                path,
                record_name: module_id,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: "js".to_string(),
                source_file: None,
                output_path: None,
                cache_file: None,
            });
        }
    }

    /// Collect module sources from ETS transform output facts.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `ark_compiler/transform.js` processing of transform
    /// output facts.
    pub fn collect_from_transform_output(
        &mut self,
        facts: impl IntoIterator<Item = TransformFactInput>,
    ) {
        for fact in facts {
            let kind = if fact.is_declaration_only {
                ModuleSourceInputKind::DeclarationOnly
            } else if fact.is_ts_for_abc {
                ModuleSourceInputKind::TsForAbc
            } else {
                ModuleSourceInputKind::NormalSource
            };
            if kind == ModuleSourceInputKind::DeclarationOnly {
                self.declaration_only.push(DeclarationOnlyPlan {
                    path: fact.output_path,
                    module_id: fact.module_id,
                });
                continue;
            }
            self.add_source(ModuleSourceFilePlan {
                kind,
                path: fact.output_path,
                record_name: fact.module_id,
                package_name: fact.package_name,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: if fact.is_ts_for_abc { "ts" } else { "js" }.to_string(),
                source_file: fact.source_file,
                output_path: None,
                cache_file: None,
            });
        }
    }

    /// Classify a single source file into its `ModuleSourceInputKind`.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `init_config.js::filterCrossplatform` and the
    /// per-file kind check in `ark_compiler/transform.js`.
    pub fn classify_source(&self, path: &Path) -> ModuleSourceInputKind {
        // Check declaration files FIRST, before checking the final extension.
        // `.d.ts` has extension `"ts"` which would otherwise match TsForAbc.
        if Self::is_declaration_path(path) {
            return ModuleSourceInputKind::DeclarationOnly;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "js" => ModuleSourceInputKind::NormalSource,
            "ts" | "ets" => ModuleSourceInputKind::TsForAbc,
            "json" | "json5" => ModuleSourceInputKind::JsonModule,
            "abc" => ModuleSourceInputKind::AbcInput,
            _ => ModuleSourceInputKind::NormalSource,
        }
    }

    fn is_declaration_path(path: &Path) -> bool {
        SourceFileName::new(path).is_declaration()
    }

    /// Process a lazy import fact and record the lazy chunk.
    pub fn process_lazy_import(&mut self, plan: LazyImportPlan) {
        self.lazy_imports.push(plan);
    }

    /// Record that a declaration-only file should be skipped for ABC.
    pub fn skip_declaration_only(&mut self, plan: DeclarationOnlyPlan) {
        self.declaration_only.push(plan);
    }

    /// Convert collected module sources to `CompileUnitPlan` entries.
    ///
    /// Declaration-only entries are excluded.  JSON modules are appended
    /// after normal sources.
    pub fn into_compile_units(mut self) -> Vec<CompileUnitPlan> {
        let mut units: Vec<CompileUnitPlan> = self
            .units
            .drain(..)
            .filter(|u| u.kind != ModuleSourceInputKind::DeclarationOnly)
            .map(|u| {
                let source = match u.kind {
                    ModuleSourceInputKind::AbcInput => ModuleSourceInput::AbcFile {
                        path: u.path.clone(),
                    },
                    _ => ModuleSourceInput::File {
                        path: u.path.clone(),
                    },
                };
                let id = CompileUnitId::new(self.next_id as usize);
                self.next_id += 1;
                CompileUnitPlan {
                    id,
                    source,
                    record_name: RecordNamePlan::Explicit(u.record_name),
                    source_file: u.source_file,
                    package_name: u.package_name,
                    is_commonjs: u.is_commonjs,
                    is_shared_module: u.is_shared_module,
                    origin_source_lang: u.origin_source_lang,
                    output_path: u.output_path,
                    cache_file: u.cache_file,
                }
            })
            .collect();
        // Append JSON module units.
        for json in self.json_modules.drain(..) {
            let id = CompileUnitId::new(self.next_id as usize);
            self.next_id += 1;
            units.push(CompileUnitPlan {
                id,
                source: ModuleSourceInput::File { path: json.path },
                record_name: RecordNamePlan::Explicit(json.record_name),
                source_file: None,
                package_name: json.package_name,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: "json".to_string(),
                output_path: None,
                cache_file: None,
            });
        }
        units
    }

    fn add_source(&mut self, plan: ModuleSourceFilePlan) {
        self.units.push(plan);
    }
}

impl Default for ModuleSourceCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_source_js_is_normal() {
        let collector = ModuleSourceCollector::new();
        assert_eq!(
            collector.classify_source(std::path::Path::new("foo.js")),
            ModuleSourceInputKind::NormalSource
        );
    }

    #[test]
    fn classify_source_dts_is_declaration() {
        let collector = ModuleSourceCollector::new();
        assert_eq!(
            collector.classify_source(std::path::Path::new("foo.d.ts")),
            ModuleSourceInputKind::DeclarationOnly
        );
    }

    #[test]
    fn collect_from_bundle_adds_units() {
        let mut collector = ModuleSourceCollector::new();
        collector.collect_from_bundle([(
            "@bundle:app/entry/Index".to_string(),
            PathBuf::from("/out/Index.js"),
        )]);
        let units = collector.into_compile_units();
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].record_name.resolve(), "@bundle:app/entry/Index");
    }

    #[test]
    fn declaration_only_facts_are_excluded() {
        let mut collector = ModuleSourceCollector::new();
        collector.collect_from_transform_output([TransformFactInput {
            module_id: "app/types".to_string(),
            output_path: PathBuf::from("/out/types.d.ts"),
            is_ts_for_abc: false,
            is_declaration_only: true,
            package_name: None,
            source_file: None,
        }]);
        let units = collector.into_compile_units();
        assert!(units.is_empty());
    }
}
