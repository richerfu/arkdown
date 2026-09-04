//! ArkUI-X module/component collection artifact writer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arkdown_core::ArkdownError;
use arkdown_ets_transform::CrossplatformSourceCollection;
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer};
use rustc_hash::FxHashMap;

/// Writes the two cache artifacts produced by ets-loader's `apiTransform`
/// lifecycle for an ArkUI-X build.
pub struct CrossplatformCollectionWriter<'a> {
    project_root: &'a Path,
    cache_path: &'a Path,
}

impl<'a> CrossplatformCollectionWriter<'a> {
    pub fn new(project_root: &'a Path, cache_path: &'a Path) -> Self {
        Self {
            project_root,
            cache_path,
        }
    }

    pub fn write(
        &self,
        sources: &FxHashMap<String, CrossplatformSourceCollection>,
    ) -> Result<(), ArkdownError> {
        let mut modules = BTreeMap::<String, Vec<String>>::new();
        let mut components = BTreeMap::<String, Vec<String>>::new();
        let mut entries = sources.iter().collect::<Vec<_>>();
        entries.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (source, collection) in entries {
            let key = self.source_key(source);
            if !collection.modules.is_empty() {
                modules.insert(key.clone(), collection.modules.clone());
            }
            if !collection.components.is_empty() {
                components.insert(key, collection.components.clone());
            }
        }
        fs_err::create_dir_all(self.cache_path).map_err(ArkdownError::Io)?;
        self.write_file("module_collection.json", &modules)?;
        self.write_file("component_collection.json", &components)?;
        Ok(())
    }

    fn source_key(&self, source: &str) -> String {
        let project_root = canonicalize_or_owned(self.project_root);
        let source_path = canonicalize_or_owned(Path::new(source));
        let relative = source_path
            .strip_prefix(project_root)
            .unwrap_or(source_path.as_path());
        ArtifactPathSerializer::rel_to_slash(relative)
    }

    fn write_file<T: serde::Serialize>(
        &self,
        file_name: &str,
        collection: &BTreeMap<String, T>,
    ) -> Result<PathBuf, ArkdownError> {
        let content = serde_json::to_vec_pretty(collection).map_err(|error| {
            ArkdownError::Bundler(format!("serialize {file_name} failed: {error}"))
        })?;
        let path = self.cache_path.join(file_name);
        fs_err::write(&path, content).map_err(ArkdownError::Io)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_project_relative_deterministic_collections() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        let cache_path = temp.path().join("cache");
        let first = project_root.join("entry/src/main/ets/pages/Index.ets");
        let second = project_root.join("entry/src/main/ets/model/Call.ts");
        fs_err::create_dir_all(first.parent().expect("first parent")).expect("first source dir");
        fs_err::create_dir_all(second.parent().expect("second parent")).expect("second source dir");
        fs_err::write(&first, "").expect("first source");
        fs_err::write(&second, "").expect("second source");

        let mut sources = FxHashMap::default();
        sources.insert(
            second.to_string_lossy().into_owned(),
            CrossplatformSourceCollection {
                modules: vec!["arkui-x.telephony".to_string()],
                components: Vec::new(),
            },
        );
        sources.insert(
            first.to_string_lossy().into_owned(),
            CrossplatformSourceCollection {
                modules: vec!["ohos.router".to_string()],
                components: vec!["Text".to_string(), "If".to_string()],
            },
        );

        CrossplatformCollectionWriter::new(&project_root, &cache_path)
            .write(&sources)
            .expect("write collections");

        assert_eq!(
            fs_err::read_to_string(cache_path.join("module_collection.json"))
                .expect("module collection"),
            concat!(
                "{\n",
                "  \"entry/src/main/ets/model/Call.ts\": [\n",
                "    \"arkui-x.telephony\"\n",
                "  ],\n",
                "  \"entry/src/main/ets/pages/Index.ets\": [\n",
                "    \"ohos.router\"\n",
                "  ]\n",
                "}"
            )
        );
        assert_eq!(
            fs_err::read_to_string(cache_path.join("component_collection.json"))
                .expect("component collection"),
            concat!(
                "{\n",
                "  \"entry/src/main/ets/pages/Index.ets\": [\n",
                "    \"Text\",\n",
                "    \"If\"\n",
                "  ]\n",
                "}"
            )
        );
    }
}
