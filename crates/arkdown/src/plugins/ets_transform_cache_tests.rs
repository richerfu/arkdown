use std::path::PathBuf;
use std::sync::Arc;

use arkdown_ets_transform::{
    CrossplatformSourceCollection, InsightIntentSourceMetadata, RolldownEtsTransformOutput,
    RolldownEtsTransformPlan, TransformSourceSnapshot,
};
use rustc_hash::FxHashMap;

use super::ets_transform::{ComponentMetadata, DiagnosticSeverity, EtsTransformError};
use super::ets_transform_cache::EtsTransformCache;
use crate::BundleCachePolicy;

fn transform_plan(path: PathBuf, source: &str) -> RolldownEtsTransformPlan {
    RolldownEtsTransformPlan::new(
        "com.example.cache",
        "entry",
        path.parent()
            .and_then(std::path::Path::parent)
            .unwrap_or_else(|| std::path::Path::new("/"))
            .to_path_buf(),
        path.parent()
            .unwrap_or_else(|| std::path::Path::new("/"))
            .to_path_buf(),
        Vec::new(),
        12,
        FxHashMap::default(),
        ComponentMetadata::default(),
        TransformSourceSnapshot::new([(path, Arc::<str>::from(source))].into_iter().collect()),
    )
}

fn output() -> RolldownEtsTransformOutput {
    RolldownEtsTransformOutput {
        code: "export const cached = 1;".to_string(),
        warnings: vec![EtsTransformError::CodedDiagnostic {
            file: "Index.ets".to_string(),
            code: "10900001".to_string(),
            message: "cached warning".to_string(),
            severity: DiagnosticSeverity::Warning,
            span: Some(oxc_span::Span::new(3, 8)),
            solutions: vec!["replace the cached expression".to_string()],
        }],
        module_source_code: Some(Arc::from("export const cached: number = 1;")),
        source_map: Some(r#"{"version":3,"sources":["Index.ets"],"mappings":"AAAA"}"#.to_string()),
        record_module_source_code: true,
        module_source_emits_javascript: false,
        emits_javascript: false,
        is_comment_only_javascript: false,
        is_shared_module: true,
        insight_intent: InsightIntentSourceMetadata::default(),
        crossplatform_collection: CrossplatformSourceCollection {
            modules: vec!["ohos.router".to_string()],
            components: vec!["Button".to_string()],
        },
    }
}

#[test]
fn cross_process_cache_restores_complete_transform_output() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module = temp.path().join("entry/src/main/ets/Index.ets");
    let module_id =
        arkdown_path::ArtifactPathSerializer::path_to_slash_string(&module).expect("module id");
    let source = "@Component struct Index { build() {} }";
    let plan = transform_plan(module, source);
    let policy = BundleCachePolicy {
        cache_directory: Some(temp.path().join("cache")),
        read_cache: true,
        write_cache: true,
        compile_env_key: "sdk#api#module".to_string(),
        transform_environment_key: "transform-v1".to_string(),
    };

    let first = EtsTransformCache::from_policy(&policy);
    first.begin_build(&plan);
    first.record(&plan, &module_id, source, &output());
    first.commit(&plan, &FxHashMap::default(), vec![module_id.clone()]);

    let second = EtsTransformCache::from_policy(&policy);
    second.begin_build(&plan);
    let restored = second
        .restore(&plan, &module_id, source)
        .expect("cross-process cache hit");
    assert_eq!(restored.code, "export const cached = 1;");
    assert_eq!(
        restored.module_source_code.as_deref(),
        Some("export const cached: number = 1;")
    );
    assert_eq!(restored.source_map, output().source_map);
    assert!(restored.is_shared_module);
    assert_eq!(restored.warnings.len(), 1);
    let EtsTransformError::CodedDiagnostic {
        span, solutions, ..
    } = &restored.warnings[0]
    else {
        panic!("expected cached coded diagnostic");
    };
    assert_eq!(*span, Some(oxc_span::Span::new(3, 8)));
    assert_eq!(solutions, &["replace the cached expression"]);
    assert_eq!(restored.crossplatform_collection.modules, ["ohos.router"]);
    assert_eq!(restored.crossplatform_collection.components, ["Button"]);
}

#[test]
fn exact_hook_input_hash_prevents_reusing_pre_plugin_output() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module = temp.path().join("entry/src/main/ets/Index.ets");
    let module_id =
        arkdown_path::ArtifactPathSerializer::path_to_slash_string(&module).expect("module id");
    let source = "export const value = 1;";
    let plan = transform_plan(module, source);
    let policy = BundleCachePolicy {
        cache_directory: Some(temp.path().join("cache")),
        read_cache: true,
        write_cache: true,
        compile_env_key: "sdk#api#module".to_string(),
        transform_environment_key: "transform-v1".to_string(),
    };
    let first = EtsTransformCache::from_policy(&policy);
    first.begin_build(&plan);
    first.record(&plan, &module_id, source, &output());
    first.commit(&plan, &FxHashMap::default(), vec![module_id.clone()]);

    let second = EtsTransformCache::from_policy(&policy);
    second.begin_build(&plan);
    assert!(second
        .restore(
            &plan,
            &module_id,
            "export const value = packagePluginRewrite();"
        )
        .is_none());
}
