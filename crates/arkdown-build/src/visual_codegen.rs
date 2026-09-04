//! Native Visual/SuperVisual source inventory and generation.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use arkdown_ets_transform::{VisualCodeGenerator, VisualGeneratedSource};
use arkdown_path::canonicalize_or_owned;
use arkdown_session::SourceStore;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

pub struct VisualCodegenAdapter<'a> {
    source_store: &'a SourceStore,
    module_roots: &'a [(String, PathBuf)],
    dependency_source_roots: &'a [PathBuf],
}

impl<'a> VisualCodegenAdapter<'a> {
    pub fn new(
        source_store: &'a SourceStore,
        module_roots: &'a [(String, PathBuf)],
        dependency_source_roots: &'a [PathBuf],
    ) -> Self {
        Self {
            source_store,
            module_roots,
            dependency_source_roots,
        }
    }

    pub fn generate(&self) -> Result<FxHashMap<PathBuf, VisualGeneratedSource>> {
        let inputs = VisualInputInventory::discover(
            self.source_store,
            self.module_roots,
            self.dependency_source_roots,
        );
        if inputs.is_empty() {
            return Ok(FxHashMap::default());
        }

        let generated = inputs
            .items
            .par_iter()
            .map(|input| {
                let source = self
                    .source_store
                    .read_text(&input.visual_path)
                    .with_context(|| {
                        format!("read Visual source {}", input.visual_path.display())
                    })?;
                let generated = VisualCodeGenerator::new(source.as_ref())
                    .generate()
                    .with_context(|| {
                        format!(
                            "ARKDOWN_VISUAL_CODEGEN_DIAGNOSTIC: {}",
                            input.visual_path.display()
                        )
                    })?;
                Ok((input.ets_path.clone(), generated))
            })
            .collect::<Vec<Result<(PathBuf, VisualGeneratedSource)>>>();

        let mut sources = FxHashMap::default();
        for generated in generated {
            let (path, source) = generated?;
            sources.insert(path, source);
        }
        Ok(sources)
    }
}

#[derive(Debug)]
struct VisualInput {
    ets_path: PathBuf,
    visual_path: PathBuf,
}

#[derive(Debug, Default)]
struct VisualInputInventory {
    items: Vec<VisualInput>,
}

impl VisualInputInventory {
    fn discover(
        source_store: &SourceStore,
        module_roots: &[(String, PathBuf)],
        dependency_source_roots: &[PathBuf],
    ) -> Self {
        let mut roots = BTreeSet::new();
        for (_, module_root) in module_roots {
            roots.insert(module_root.join("src/main/supervisual"));
        }
        for root in dependency_source_roots {
            for candidate in Self::dependency_visual_roots(root) {
                roots.insert(candidate);
            }
        }

        let roots = roots
            .into_iter()
            .map(|root| canonicalize_or_owned(&root))
            .collect::<Vec<_>>();
        let mut items = Vec::new();
        for (ets_path, visual_path) in source_store.visual_sources() {
            if roots.iter().any(|root| visual_path.starts_with(root))
                && source_store.source_by_path(ets_path).is_some()
            {
                items.push(VisualInput {
                    ets_path: canonicalize_or_owned(ets_path),
                    visual_path: canonicalize_or_owned(visual_path),
                });
            }
        }
        items.sort_by(|left, right| {
            left.ets_path
                .cmp(&right.ets_path)
                .then_with(|| left.visual_path.cmp(&right.visual_path))
        });
        items.dedup_by(|left, right| left.ets_path == right.ets_path);
        Self { items }
    }

    fn dependency_visual_roots(root: &Path) -> Vec<PathBuf> {
        let mut roots = vec![root.join("src/main/supervisual")];
        if root.file_name().is_some_and(|name| name == "ets") {
            if let Some(src_main) = root.parent() {
                roots.push(src_main.join("supervisual"));
            }
        }
        roots
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_maps_supervisual_to_existing_ets_sources_deterministically() {
        let temp = tempfile::tempdir().expect("tempdir");
        let module_root = temp.path().join("entry");
        let ets_root = module_root.join("src/main/ets/pages");
        let visual_root = module_root.join("src/main/supervisual/pages");
        std::fs::create_dir_all(&ets_root).expect("ets root");
        std::fs::create_dir_all(&visual_root).expect("visual root");
        let source = ets_root.join("Index.ets");
        let visual = visual_root.join("Index.visual");
        std::fs::write(&source, "@Component struct Index { build() {} }").expect("source");
        std::fs::write(&visual, "{}").expect("visual");
        std::fs::write(visual_root.join("Orphan.visual"), "{}").expect("orphan");

        let source_store =
            SourceStore::discover(&[module_root.join("src/main/ets")]).expect("source store");
        let inventory = VisualInputInventory::discover(
            &source_store,
            &[("entry".to_string(), module_root)],
            &[],
        );

        assert_eq!(inventory.items.len(), 1);
        assert_eq!(inventory.items[0].ets_path, canonicalize_or_owned(&source));
        assert_eq!(
            inventory.items[0].visual_path,
            canonicalize_or_owned(&visual)
        );
    }

    #[test]
    fn native_codegen_batch_returns_typed_fragments() {
        let temp = tempfile::tempdir().expect("tempdir");
        let module_root = temp.path().join("entry");
        let ets_root = module_root.join("src/main/ets/pages");
        let visual_root = module_root.join("src/main/supervisual/pages");
        std::fs::create_dir_all(&ets_root).expect("ets root");
        std::fs::create_dir_all(&visual_root).expect("visual root");
        let source = ets_root.join("Index.ets");
        std::fs::write(&source, "@Component struct Index { build() {} }").expect("source");
        std::fs::write(
      visual_root.join("Index.visual"),
      r#"{
        "document":{"VisualVersion":"12","type":"ETS","custom":false,"path":"Index.visual"},
        "visualModel":{"dataType":"VisualModel","value":{"id":"root","type":"column","property":{"dataType":"Map","value":{}},"dynamicProperty":{"dataType":"Map","value":{}},"children":[]}}
      }"#,
    )
    .expect("visual source");

        let source_store =
            SourceStore::discover(&[module_root.join("src/main/ets")]).expect("source store");
        let generated =
            VisualCodegenAdapter::new(&source_store, &[("entry".to_string(), module_root)], &[])
                .generate()
                .expect("Visual generation");

        assert_eq!(
            generated
                .get(&canonicalize_or_owned(&source))
                .map(|fragments| fragments.build.as_str()),
            Some("    Column()")
        );
    }
}
