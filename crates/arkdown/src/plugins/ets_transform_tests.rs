use super::*;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct CapturedSource {
    path: PathBuf,
    text: Arc<str>,
}
impl arkdown_core::SourceInventory for CapturedSource {
    fn canonical_path(&self, path: &Path) -> Option<&Path> {
        (path == self.path).then_some(self.path.as_path())
    }
    fn source_text(&self, path: &Path) -> Result<Option<Arc<str>>, arkdown_core::SourceReadError> {
        Ok((path == self.path).then(|| Arc::clone(&self.text)))
    }
}

fn plan(path: &Path, text: &str) -> Arc<RolldownEtsTransformPlan> {
    Arc::new(RolldownEtsTransformPlan::new(
        "com.example",
        "entry",
        path.parent().unwrap().into(),
        path.parent().unwrap().into(),
        vec![],
        12,
        FxHashMap::default(),
        ComponentMetadata::default(),
        TransformSourceSnapshot::default().with_source_inventory(Arc::new(CapturedSource {
            path: path.into(),
            text: Arc::from(text),
        })),
    ))
}

#[test]
fn load_uses_original_snapshot_bytes_and_published_incremental_version() {
    let temp = tempfile::tempdir().unwrap();
    for extension in ["ets", "ts", "js", "mjs", "cjs"] {
        let path = temp.path().join(format!("source.{extension}"));
        let text = "\u{feff}// 原始\r\nexport const x = 1;\r\n";
        std::fs::write(&path, text).unwrap();
        let captured = std::fs::read_to_string(&path).unwrap();
        let plugin =
            EtsTransformPlugin::from_plan(plan(&path, &captured), &BundleCachePolicy::default());
        assert_eq!(
            plugin.load_meta().unwrap().order,
            Some(rolldown_plugin::PluginOrder::PinPost)
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            plugin
                .load_source(path.to_str().unwrap())
                .unwrap()
                .unwrap()
                .code
                .as_str(),
            text
        );
        plugin.update_plan(plan(&path, "export const x = 2;"));
        assert_eq!(
            plugin
                .load_source(path.to_str().unwrap())
                .unwrap()
                .unwrap()
                .code
                .as_str(),
            "export const x = 2;"
        );
    }
    for name in ["data.json", "data.json5", "\0ignored", "outside.ts"] {
        let path = temp.path().join(name);
        let plugin = EtsTransformPlugin::from_plan(
            plan(&temp.path().join("source.ts"), "source"),
            &BundleCachePolicy::default(),
        );
        assert!(plugin
            .load_source(path.to_str().unwrap())
            .unwrap()
            .is_none());
    }
}
