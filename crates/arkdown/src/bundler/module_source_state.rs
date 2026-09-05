//! hvigor module source state materialized from a Rolldown module-source scan.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arcstr::ArcStr;
use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_core::ChunkGraph;
use arkdown_ets_transform::{CrossplatformSourceCollection, InsightIntentSourceMetadata};
use oxc_index::IndexVec;
use rolldown::ModuleSourceScanOutput;

use crate::chunk_module_request::HvigorChunkModuleRequestProcessor;

use super::*;

pub(crate) struct HvigorExternalModuleRequest<'a> {
    pub(crate) request: &'a str,
}

impl<'a> HvigorExternalModuleRequest<'a> {
    pub(crate) fn new(request: &'a str) -> Self {
        Self { request }
    }

    pub(crate) fn is_match(&self) -> bool {
        self.request.starts_with("@ohos:")
            || self.request.starts_with("@ohos.")
            || self.request.starts_with("@hms:")
            || self.request.starts_with("@hms.")
            || self.request.starts_with("@native:")
            || self.request.starts_with("@native.")
            || self.request.starts_with("@app:")
            || self.request.starts_with("@system.")
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HvigorModuleSourceState {
    pub(crate) sources: FxHashMap<String, HvigorCollectedModuleSource>,
    pub(crate) source_maps: FxHashMap<String, Arc<str>>,
    pub(crate) resolved_imports: FxHashMap<String, FxHashMap<String, String>>,
    pub(crate) resolved_import_records: FxHashMap<String, Vec<HvigorResolvedImportRecord>>,
    pub(crate) module_ids: Vec<String>,
    pub(crate) seen_module_ids: FxHashSet<String>,
    pub(crate) full_module_is_commonjs: FxHashMap<String, bool>,
    pub(crate) javascript_module_ids: FxHashSet<String>,
    pub(crate) shared_module_ids: FxHashSet<String>,
    pub(crate) insight_intent_sources: FxHashMap<String, InsightIntentSourceMetadata>,
    pub(crate) crossplatform_collections: FxHashMap<String, CrossplatformSourceCollection>,
}

impl HvigorModuleSourceState {
    pub(crate) fn from_rolldown_scan(
        scan: ModuleSourceScanOutput,
        mut transform: HvigorModuleTransformSnapshot,
    ) -> Self {
        let mut state = Self {
            source_maps: transform.source_maps,
            javascript_module_ids: transform.javascript_module_ids,
            shared_module_ids: transform.shared_module_ids,
            insight_intent_sources: transform.insight_intent_sources,
            crossplatform_collections: transform.crossplatform_collections,
            ..Self::default()
        };
        for module in scan.modules {
            let module_id = module.id.to_string();
            state.record_module_id(module_id.clone());
            let source = transform
                .module_source_overrides
                .remove(&module_id)
                .map(|source| ArcStr::from(source.as_ref()))
                .unwrap_or(module.source);
            state.sources.insert(
                module_id.clone(),
                HvigorCollectedModuleSource { code: source },
            );
            state
                .full_module_is_commonjs
                .insert(module_id.clone(), module.exports_kind.is_commonjs());

            for import in module.imports {
                let resolved_id = import
                    .resolved_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| import.module_request.clone());
                state.record_module_id(resolved_id.clone());
                state
                    .resolved_imports
                    .entry(module_id.clone())
                    .or_default()
                    .insert(import.module_request.clone(), resolved_id);
                state
                    .resolved_import_records
                    .entry(module_id.clone())
                    .or_default()
                    .push(HvigorResolvedImportRecord {
                        specifier: import.module_request,
                    });
            }
        }
        state
    }

    pub(crate) fn take_insight_intent_sources(
        &mut self,
    ) -> FxHashMap<String, InsightIntentSourceMetadata> {
        std::mem::take(&mut self.insight_intent_sources)
    }

    pub(crate) fn take_crossplatform_collections(
        &mut self,
    ) -> FxHashMap<String, CrossplatformSourceCollection> {
        std::mem::take(&mut self.crossplatform_collections)
    }

    pub(crate) fn record_module_id(&mut self, id: String) {
        if self.seen_module_ids.insert(id.clone()) {
            self.module_ids.push(id);
        }
    }

    pub(crate) fn module_graph_facts(&self) -> ModuleGraphFacts {
        ModuleGraphFacts {
            // `record_module_id` already de-duplicates while retaining the
            // module scan's insertion order. Sorting here changes filesInfo,
            // proto cache and merged ABC input order relative to hvigor.
            full_module_ids: self.module_ids.clone(),
            full_module_is_commonjs: self.full_module_is_commonjs.clone(),
        }
    }

    pub(crate) fn module_imports(&self) -> FxHashMap<String, Vec<String>> {
        let mut imports = FxHashMap::default();
        for (importer, resolved_imports) in &self.resolved_imports {
            let mut imported_ids = resolved_imports.values().cloned().collect::<Vec<_>>();
            imported_ids.sort();
            imported_ids.dedup();
            imports.insert(importer.clone(), imported_ids);
        }
        imports
    }

    pub(crate) fn external_module_requests(&self) -> Vec<String> {
        let mut requests = Vec::new();
        for resolved_imports in self.resolved_imports.values() {
            for (specifier, resolved_id) in resolved_imports {
                if HvigorExternalModuleRequest::new(specifier).is_match() {
                    requests.push(specifier.clone());
                } else if HvigorExternalModuleRequest::new(resolved_id).is_match() {
                    requests.push(resolved_id.clone());
                }
            }
        }
        requests.sort();
        requests.dedup();
        requests
    }

    pub(crate) fn har_importees(
        &self,
        module_path_map: &HvigorModulePathMap,
        current_module_name: &str,
    ) -> HvigorHarImportees {
        let mut importees = HvigorHarImportees::default();
        let mut seen_compiled = FxHashSet::default();
        let mut seen_direct = FxHashSet::default();
        let mut seen_non_entry = FxHashSet::default();

        // `process-importee.js::collectImportees` walks Rolldown modules and
        // each module's resolvedIds in encounter order. `module_ids` retains
        // that scan order while `resolved_import_records` retains request
        // order without depending on FxHashMap iteration.
        for module_id in &self.module_ids {
            let Some(records) = self.resolved_import_records.get(module_id) else {
                continue;
            };
            let belongs_to_current_module = module_path_map
                .source_file_for_entry(module_id)
                .is_ok_and(|source| {
                    source.module_name == current_module_name
                        && matches!(source.kind, HvigorChunkSourceKind::Source)
                });
            for record in records {
                let request = HvigorImporteeRequest::new(&record.specifier);
                if request.is_valid(false) && seen_compiled.insert(record.specifier.clone()) {
                    importees.compiled.push(record.specifier.clone());
                }
                if !belongs_to_current_module {
                    continue;
                }
                if request.is_valid(true) && seen_non_entry.insert(record.specifier.clone()) {
                    importees.non_entry.push(record.specifier.clone());
                }
                if request.is_valid(false) && seen_direct.insert(record.specifier.clone()) {
                    importees.direct.push(record.specifier.clone());
                }
            }
        }
        importees
    }

    pub(crate) fn preload_system_so_tasks(
        &self,
        entry_files: &[PathBuf],
        processor: &HvigorChunkModuleRequestProcessor,
    ) -> Vec<PreloadSystemSoTask> {
        if entry_files.is_empty() {
            return Vec::new();
        }

        let mut module_ids_by_path = FxHashMap::<PathBuf, String>::default();
        for module_id in &self.module_ids {
            let module_id = module_id.strip_prefix('\0').unwrap_or(module_id);
            let path = Path::new(module_id);
            if path.has_root() {
                module_ids_by_path.insert(
                    arkdown_path::canonicalize_or_owned(path),
                    module_id.to_string(),
                );
            }
        }

        let mut pending = entry_files
            .iter()
            .filter_map(|entry| {
                module_ids_by_path
                    .get(&arkdown_path::canonicalize_or_owned(entry))
                    .cloned()
            })
            .collect::<Vec<_>>();
        let mut reachable = FxHashSet::default();
        while let Some(module_id) = pending.pop() {
            if !reachable.insert(module_id.clone()) {
                continue;
            }
            if let Some(imports) = self.resolved_imports.get(&module_id) {
                for resolved_id in imports.values() {
                    if self.sources.contains_key(resolved_id) {
                        pending.push(resolved_id.clone());
                    }
                }
            }
        }

        let mut reachable = reachable.into_iter().collect::<Vec<_>>();
        reachable.sort();
        let mut seen_names = FxHashSet::default();
        let mut tasks = Vec::new();
        for module_id in reachable {
            let Some(records) = self.resolved_import_records.get(&module_id) else {
                continue;
            };
            for record in records {
                let Some(ohm_url) =
                    PreloadSystemSoRequest::new(&record.specifier, processor).to_ohm_url()
                else {
                    continue;
                };
                if !seen_names.insert(record.specifier.clone()) {
                    continue;
                }
                tasks.push(PreloadSystemSoTask {
                    name: record.specifier.clone(),
                    src_entry: record.specifier.clone(),
                    ohm_url,
                });
            }
        }
        tasks
    }
}

pub(crate) struct HvigorImporteeRequest<'a> {
    pub(crate) value: &'a str,
}

impl<'a> HvigorImporteeRequest<'a> {
    pub(crate) fn new(value: &'a str) -> Self {
        Self { value }
    }

    /// Exact shape of latest `process-importee.js::isValidImportee`.
    pub(crate) fn is_valid(&self, require_subpath: bool) -> bool {
        if self.value.is_empty()
            || self.value.contains('\0')
            || Path::new(self.value).is_absolute()
            || self.value.starts_with('.')
        {
            return false;
        }
        if !require_subpath {
            return true;
        }
        let segment_count = self
            .value
            .split(['/', '\\'])
            .filter(|segment| !segment.is_empty())
            .count();
        segment_count > usize::from(self.value.starts_with('@')) + 1
    }
}

pub(crate) struct PreloadSystemSoRequest<'a> {
    pub(crate) request: &'a str,
    pub(crate) processor: &'a HvigorChunkModuleRequestProcessor,
}

impl<'a> PreloadSystemSoRequest<'a> {
    pub(crate) fn new(request: &'a str, processor: &'a HvigorChunkModuleRequestProcessor) -> Self {
        Self { request, processor }
    }

    pub(crate) fn to_ohm_url(&self) -> Option<String> {
        let ohm_url = self.processor.process_request(self.request);
        if ohm_url != self.request || Self::is_pre_normalized_system_request(self.request) {
            Some(ohm_url)
        } else {
            None
        }
    }

    pub(crate) fn is_pre_normalized_system_request(request: &str) -> bool {
        request.starts_with("@ohos:")
            || request.starts_with("@hms:")
            || request.starts_with("@native:")
    }
}

#[cfg(feature = "memory-diagnostics")]
impl HvigorModuleSourceState {
    pub(crate) fn memory_summary(&self) -> String {
        let source_bytes = self
            .sources
            .values()
            .map(|source| source.code.len())
            .sum::<usize>();
        let source_map_bytes = self
            .source_maps
            .values()
            .map(|source_map| source_map.len())
            .sum::<usize>();
        let resolved_imports = self
            .resolved_imports
            .values()
            .map(|imports| imports.len())
            .sum::<usize>();
        let resolved_import_records = self
            .resolved_import_records
            .values()
            .map(Vec::len)
            .sum::<usize>();
        format!(
            "sources={} source_bytes={} source_maps={} source_map_bytes={} module_ids={} resolved_importers={} resolved_imports={} resolved_import_records={} pending_resolved_imports={} full_commonjs={}",
            self.sources.len(),
            source_bytes,
            self.source_maps.len(),
            source_map_bytes,
            self.module_ids.len(),
            self.resolved_imports.len(),
            resolved_imports,
            resolved_import_records,
            0,
            self.full_module_is_commonjs.len()
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HvigorCollectedModuleSource {
    pub(crate) code: ArcStr,
}

#[derive(Debug, Clone)]
pub(crate) struct HvigorResolvedImportRecord {
    pub(crate) specifier: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HvigorSourceSpan {
    pub(crate) start: u32,
    pub(crate) end: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleGraphFacts {
    pub(crate) full_module_ids: Vec<String>,
    pub(crate) full_module_is_commonjs: FxHashMap<String, bool>,
}

impl ModuleGraphFacts {
    pub(crate) fn chunk_graph(&self) -> ChunkGraph {
        ChunkGraph {
            chunks: IndexVec::new(),
            full_module_ids: self.full_module_ids.clone(),
            full_module_is_commonjs: self.full_module_is_commonjs.clone(),
            module_to_chunk: IndexVec::new(),
            entry_module_to_entry_chunk: FxHashMap::default(),
        }
    }
}
