//! Rolldown input planning from a `BundlePlan`.

use std::path::PathBuf;

use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_core::{ArkdownError, ChunkSourceFile};
use arkdown_path::{
    lexical_normalize, specifier::SpecifierKind, strip_relative_prefix, AbsPath, ModuleSpecifier,
};
use rolldown::InputItem;

use crate::compile_entry::CompileEntryNamer;
use crate::plan::BundlePlan;

use super::*;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

pub(crate) struct RolldownInputPlan {
    pub(crate) module_root: PathBuf,
    pub(crate) module_path_map: HvigorModulePathMap,
    pub(crate) entries: Vec<String>,
    pub(crate) namer: CompileEntryNamer,
}

impl RolldownInputPlan {
    pub(crate) fn from_bundle_plan(plan: &BundlePlan) -> Self {
        let module_root = Self::canonical_module_root(&plan.input.module_root);
        let mut module_path_map = HvigorModulePathMap::new(
            &plan.input.module_name,
            &module_root,
            &plan.input.module_path_map,
        );
        module_path_map.add_module_record_roots(&plan.input.module_record_roots);
        module_path_map.add_module_meta_info_files(&plan.input.module_meta_info_files);
        let entries = plan.input.entries.clone();
        module_path_map.add_compile_entry_package_roots(&entries);
        Self {
            module_root,
            module_path_map,
            namer: CompileEntryNamer::new(&entries),
            entries,
        }
    }

    pub(crate) fn input_items(&self) -> Result<Vec<InputItem>, ArkdownError> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Ok(InputItem {
                    name: Some(self.namer.chunk_name(entry, index)),
                    import: self.import_path(entry)?,
                })
            })
            .collect()
    }

    pub(crate) fn chunk_source_files(
        &self,
    ) -> Result<FxHashMap<String, ChunkSourceFile>, ArkdownError> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let chunk_name = self.namer.chunk_name(entry, index);
                let source = self.module_path_map.source_file_for_entry(entry)?;
                Ok((chunk_name.clone(), source.with_chunk_name(chunk_name)))
            })
            .collect()
    }

    pub(crate) fn entry_module_ids(&self) -> Result<Vec<String>, ArkdownError> {
        let mut entry_module_ids = Vec::with_capacity(self.entries.len());
        let mut seen = FxHashSet::default();
        for entry in &self.entries {
            let module_id = self.import_path(entry)?;
            if seen.insert(module_id.clone()) {
                entry_module_ids.push(module_id);
            }
        }
        Ok(entry_module_ids)
    }

    pub(crate) fn chunk_source_file_map(
        &self,
        sources: &FxHashMap<String, ChunkSourceFile>,
    ) -> FxHashMap<String, String> {
        sources
            .iter()
            .map(|(chunk_name, source)| (chunk_name.clone(), source.source_file.clone()))
            .collect()
    }

    pub(crate) fn import_path(&self, entry: &str) -> Result<String, ArkdownError> {
        let specifier = ModuleSpecifier::new(entry);
        if matches!(
            specifier.kind(),
            SpecifierKind::AbsolutePath | SpecifierKind::OhmUrl | SpecifierKind::ScopedPackage
        ) {
            return Ok(entry.to_string());
        }

        let candidate = self.module_root.join(strip_relative_prefix(entry));
        AbsPath::canonicalize(&candidate)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| lexical_normalize(&candidate))
            .into_os_string()
            .into_string()
            .map_err(|path| {
                ArkdownError::Bundler(format!(
                    "bundle entry path is not valid UTF-8: {}",
                    path.display()
                ))
            })
    }

    pub(crate) fn canonical_module_root(path: &std::path::Path) -> PathBuf {
        AbsPath::canonicalize(path)
            .map(AbsPath::into_path_buf)
            .unwrap_or_else(|_| {
                if path.has_root() {
                    lexical_normalize(path)
                } else {
                    lexical_normalize(
                        &std::env::current_dir()
                            .unwrap_or_else(|_| PathBuf::from("."))
                            .join(path),
                    )
                }
            })
    }
}
