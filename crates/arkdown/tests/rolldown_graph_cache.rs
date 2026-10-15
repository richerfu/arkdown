use std::borrow::Cow;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use arkdown::{
    BundleCachePolicy, BundleInput, BundleOutputOptions, BundlePlan, BundlePluginSet,
    BundleRuntimePackageContext, BundleRuntimePackageInfo, BundleSourceMapPackageInfo, Bundler,
    LoaderResourceJoin, SourceMapPolicy,
};
use arkdown_core::{ArktsLanguageVersion, BuildMode, OutputTarget};
use rolldown_plugin::{
    HookCloseBundleArgs, HookTransformArgs, HookTransformReturn, HookUsage, Plugin, PluginContext,
    SharedTransformPluginContext,
};
use rustc_hash::FxHashMap;

#[derive(Debug)]
struct TransformProbe {
    source_root: PathBuf,
    transform_count: Arc<AtomicUsize>,
    close_count: Arc<AtomicUsize>,
}

impl Plugin for TransformProbe {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("arkdown-incremental-transform-probe")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Transform | HookUsage::CloseBundle
    }

    fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        args: &HookTransformArgs<'_>,
    ) -> impl Future<Output = HookTransformReturn> + Send {
        // Rolldown may retain the Windows short spelling of the temp directory.
        if arkdown_path::AbsPath::canonicalize(Path::new(args.id))
            .is_ok_and(|path| path.starts_with(&self.source_root))
        {
            self.transform_count.fetch_add(1, Ordering::Relaxed);
        }
        async { Ok(None) }
    }

    fn close_bundle(
        &self,
        _ctx: &PluginContext,
        _args: Option<&HookCloseBundleArgs>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        self.close_count.fetch_add(1, Ordering::Relaxed);
        async { Ok(()) }
    }
}

fn graph_plan(root: &Path, entry: &Path) -> BundlePlan {
    let module_root = root.join("entry");
    let source_root = module_root.join("src/main/ets");
    let loader_json = module_root.join("build/default/intermediates/loader/loader.json");
    let cache_dir =
        module_root.join("build/default/cache/default/default@CompileArkTS/esmodule/debug");
    let mut runtime_packages = FxHashMap::default();
    runtime_packages.insert(
        "entry".to_string(),
        BundleRuntimePackageContext {
            package_name: "entry".to_string(),
            bundle_name: "com.example.cache".to_string(),
            module_name: "entry".to_string(),
            version: "1.0.0".to_string(),
            entry_path: "Index.js".to_string(),
            is_so: false,
        },
    );
    BundlePlan {
        input: BundleInput {
            loader_json_path: loader_json.clone(),
            project_root: root.to_path_buf(),
            project_path: source_root,
            module_root: module_root.clone(),
            module_name: "entry".to_string(),
            module_path_map: vec![("entry".to_string(), module_root.clone())],
            module_record_roots: vec![("entry".to_string(), module_root)],
            module_meta_info_files: Vec::new(),
            entries: vec![entry.to_str().expect("UTF-8 test entry").to_string()],
            form_entry_sources: Vec::new(),
            insight_intent_form_bindings: Vec::new(),
            runtime_entry_sources: Vec::new(),
            page_entry_sources: Vec::new(),
            support_chunks: false,
            source_map_packages: BundleSourceMapPackageInfo {
                entry_package_name: "entry".to_string(),
                entry_package_version: "1.0.0".to_string(),
                dependency_package_versions: FxHashMap::default(),
            },
            runtime_packages: BundleRuntimePackageInfo {
                packages: runtime_packages,
            },
            byte_code_har_packages: Vec::new(),
            byte_code_har: false,
            preload_system_so_entries: Vec::new(),
            mock_config: None,
        },
        output: BundleOutputOptions {
            output_dir: root.join("output"),
            module_source_output_dir: Some(cache_dir),
            ..BundleOutputOptions::default()
        },
        source_map: SourceMapPolicy::None,
        cache: BundleCachePolicy::default(),
        loader_resource: LoaderResourceJoin {
            loader_json_path: loader_json,
            resources_index_path: None,
            resource_table_path: None,
        },
        plugins: BundlePluginSet::default(),
        build_mode: BuildMode::Debug,
        arkts_language_version: ArktsLanguageVersion::V1_1,
        target_api_version: 12,
        compatible_sdk_version: "12".to_string(),
        runtime_os: "OpenHarmony".to_string(),
        bundle_name: "com.example.cache".to_string(),
        use_normalized_ohm_url: false,
        should_emit_js: true,
        output_target: OutputTarget::Hap,
        preview_mode: false,
        check_entry: false,
        shim_missing_exports_override: None,
        treeshake_override: Some(false),
        context_override: None,
    }
}

#[tokio::test]
async fn retained_graph_only_retransforms_changed_modules_and_closes_once() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let module_root = root.join("entry");
    let source_root = module_root.join("src/main/ets");
    let entry = source_root.join("Index.js");
    let dependency = source_root.join("Dependency.js");
    let loader_json = module_root.join("build/default/intermediates/loader/loader.json");
    fs_err::create_dir_all(&source_root).expect("source directory");
    fs_err::create_dir_all(loader_json.parent().expect("loader parent")).expect("loader directory");
    fs_err::write(&loader_json, "{}").expect("loader json");
    fs_err::write(
        module_root.join("oh-package.json5"),
        r#"{"name":"entry","version":"1.0.0","main":"src/main/ets/Index.js"}"#,
    )
    .expect("package manifest");
    fs_err::write(
        &entry,
        "import { value } from './Dependency.js'; export const result = value;\n",
    )
    .expect("entry source");
    fs_err::write(&dependency, "export const value = 1;\n").expect("dependency source");

    let transform_count = Arc::new(AtomicUsize::new(0));
    let close_count = Arc::new(AtomicUsize::new(0));
    let plugin = Arc::new(TransformProbe {
        source_root: arkdown_path::AbsPath::canonicalize(&source_root)
            .expect("canonical source root")
            .into_path_buf(),
        transform_count: Arc::clone(&transform_count),
        close_count: Arc::clone(&close_count),
    });
    let mut bundler = Bundler::new(vec![plugin]);
    let plan = graph_plan(root, &entry);

    let first = bundler
        .bundle_incremental(&plan, &[])
        .await
        .expect("initial retained full scan");
    assert_eq!(transform_count.load(Ordering::Relaxed), 2);
    assert_eq!(first.module_source_files.len(), 2);

    fs_err::write(&dependency, "export const value = 2;\n").expect("changed dependency source");
    let second = bundler
        .bundle_incremental(&plan, std::slice::from_ref(&dependency))
        .await
        .expect("incremental partial scan");
    assert_eq!(
        transform_count.load(Ordering::Relaxed),
        3,
        "the unchanged entry must remain in Rolldown's graph cache"
    );
    assert_eq!(second.module_source_files.len(), 2);
    let dependency_output = second
        .module_source_files
        .iter()
        .find(|source| {
            arkdown_path::AbsPath::canonicalize(Path::new(&source.module_id))
                .expect("canonical module source")
                == arkdown_path::AbsPath::canonicalize(&dependency).expect("canonical dependency")
        })
        .expect("dependency output");
    assert!(fs_err::read_to_string(&dependency_output.path)
        .expect("transformed dependency")
        .contains("value = 2"));

    bundler.close().await.expect("close retained graph");
    assert_eq!(close_count.load(Ordering::Relaxed), 1);
}
