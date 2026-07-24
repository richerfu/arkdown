use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::{
  ArkdownConfig, BuildMode, BuildOption, HarArtifactType, ModuleConfig, NativeLibOption,
  OhosModuleType, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_session::dependency_graph::snapshot::{
  DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};
use arkdown_session::ProjectSnapshot;
use indexmap::IndexMap;

use super::har_dedup::HarDedupProjectPlan;

#[test]
fn hsp_collects_only_common_source_har_roots() {
  let root = PathBuf::from("/project");
  let common_root = root.join("oh_modules/common-har");
  let (config, mut project) = fixture(&root);
  project.dependency_graphs.insert(
    "entry".to_string(),
    Arc::new(graph(
      "entry",
      vec![
        node(
          "shared",
          &root.join("shared"),
          DependencyNodeType::SharedHsp,
          false,
        ),
        node(
          "common-har",
          &common_root,
          DependencyNodeType::SourceHar,
          false,
        ),
      ],
    )),
  );
  project.dependency_graphs.insert(
    "shared".to_string(),
    Arc::new(graph(
      "shared",
      vec![node(
        "common-har",
        &common_root,
        DependencyNodeType::SourceHar,
        false,
      )],
    )),
  );

  let plan = HarDedupProjectPlan::collect(&config, &project).expect("collect dedup topology");
  let shared = plan.module("shared");

  assert_eq!(shared.source_har_roots(), &[common_root]);
  assert!(!shared.is_empty());
}

#[test]
fn direct_hsp_common_dependency_must_be_direct_on_hap() {
  let root = PathBuf::from("/project");
  let common_root = root.join("oh_modules/common-har");
  let (config, mut project) = fixture(&root);
  project.dependency_graphs.insert(
    "entry".to_string(),
    Arc::new(graph(
      "entry",
      vec![
        node(
          "shared",
          &root.join("shared"),
          DependencyNodeType::SharedHsp,
          false,
        ),
        node(
          "common-har",
          &common_root,
          DependencyNodeType::SourceHar,
          true,
        ),
      ],
    )),
  );
  project.dependency_graphs.insert(
    "shared".to_string(),
    Arc::new(graph(
      "shared",
      vec![node(
        "common-har",
        &common_root,
        DependencyNodeType::SourceHar,
        false,
      )],
    )),
  );

  let error = HarDedupProjectPlan::collect(&config, &project)
    .err()
    .expect("missing direct HAP dependency must fail")
    .to_string();

  assert!(error.contains("common-har"), "{error}");
  assert!(
    error.contains("must be declared in dependencies of entry"),
    "{error}"
  );
}

fn fixture(root: &Path) -> (ArkdownConfig, ProjectSnapshot) {
  let config = ArkdownConfig {
    project_root: root.to_path_buf(),
    build_mode: BuildMode::Debug,
    output_target: OutputTarget::Hsp,
    product: ProductConfig {
      name: "default".to_string(),
      bundle_name: "com.example.app".to_string(),
      compatible_sdk_version: "12".to_string(),
      compatible_sdk_version_stage: None,
      target_sdk_version: None,
      runtime_os: "OpenHarmony".to_string(),
      signing_config: None,
      build_option: BuildOption {
        strict_mode: StrictModeConfig {
          case_sensitive_check: false,
          use_normalized_ohm_url: true,
        },
        ark_options: None,
        res_options: None,
        source_option: None,
        preload_system_so: false,
        preload_so: Vec::new(),
        deduplicate_har: true,
        remove_permissions: Vec::new(),
        aot_compile_mode: None,
        host_pgo: None,
        ap_path: None,
        abi_filters: Vec::new(),
        external_native_options: None,
        har_artifact_type: HarArtifactType::Original,
        native_lib: NativeLibOption::default(),
      },
      version_code: None,
      version_name: None,
      build_version: None,
      app_options: Default::default(),
    },
    modules: vec![
      ModuleConfig {
        name: "entry".to_string(),
        src_path: root.join("entry"),
        module_type: OhosModuleType::Entry,
      },
      ModuleConfig {
        name: "shared".to_string(),
        src_path: root.join("shared"),
        module_type: OhosModuleType::Shared,
      },
    ],
    sdk_config: SdkConfig {
      ets_loader_path: root.join("sdk/ets-loader"),
      es2abc_path: root.join("sdk/es2abc"),
      restool_path: root.join("sdk/restool"),
      compile_api_version: 12,
      has_rollup_plugin: true,
      compile_sdk_version: "6.0.0.0".to_string(),
    },
  };
  let project = ProjectSnapshot {
    root: root.to_path_buf(),
    bundle_name: config.product.bundle_name.clone(),
    version_code: 1,
    version_name: "1.0.0".to_string(),
    build_version: None,
    bundle_type: "app".to_string(),
    product_name: config.product.name.clone(),
    module_order: vec!["entry".to_string(), "shared".to_string()],
    modules: IndexMap::new(),
    root_package: None,
    dependency_graphs: IndexMap::new(),
  };
  (config, project)
}

fn graph(host_module: &str, nodes: Vec<DependencyNodeSnapshot>) -> DependencyGraphSnapshot {
  DependencyGraphSnapshot {
    host_module: host_module.to_string(),
    nodes: nodes
      .into_iter()
      .map(|node| (node.package_name.clone(), node))
      .collect(),
    ..Default::default()
  }
}

fn node(
  package_name: &str,
  root: &Path,
  node_type: DependencyNodeType,
  is_transitive: bool,
) -> DependencyNodeSnapshot {
  DependencyNodeSnapshot {
    alias: package_name.to_string(),
    package_name: package_name.to_string(),
    version_spec: Some("1.0.0".to_string()),
    package_root: Some(root.to_path_buf()),
    package: None,
    module_profile: None,
    node_type,
    is_runtime_only: false,
    is_dynamic: false,
    is_transitive,
    direct_deps: Vec::new(),
  }
}
