//! Dependency-derived loader.json data collection.

use super::*;

impl LoaderJson {
    pub(super) fn read_package_main(module_root: &Path) -> Option<String> {
        let path = module_root.join("oh-package.json5");
        let content = std::fs::read_to_string(path).ok()?;

        #[derive(serde::Deserialize)]
        struct PackageMain {
            main: Option<String>,
        }

        json5::from_str::<PackageMain>(&content).ok()?.main
    }

    pub(super) fn read_loader_router_maps(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
        router_map_ref: Option<&str>,
        graph: &crate::dependency_graph::LocalDependencyGraph,
        pkg_contexts: Option<&std::collections::BTreeMap<String, PkgContextInfoOpt>>,
    ) -> Vec<serde_json::Value> {
        let mut router_maps = Vec::new();
        let host_router_module = RouterMapModule {
            module_name: host_module.name.clone(),
            package_name: Self::dependency_package_name(&host_module.src_path),
            src_path: host_module.src_path.clone(),
        };
        router_maps.extend(Self::read_loader_router_map(
            &config.product.bundle_name,
            &host_module.name,
            &host_router_module,
            router_map_ref,
            None,
        ));

        for router_module in Self::dependency_router_map_modules(config, host_module, graph) {
            let Some(router_map_ref) = Self::read_module_router_map_ref(&router_module.src_path)
            else {
                continue;
            };
            router_maps.extend(Self::read_loader_router_map(
                &config.product.bundle_name,
                &host_module.name,
                &router_module,
                Some(&router_map_ref),
                pkg_contexts.and_then(|contexts| contexts.get(&router_module.package_name)),
            ));
        }

        router_maps
    }

    pub(super) fn dependency_router_map_modules(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<RouterMapModule> {
        let mut modules = Vec::new();
        let mut seen_roots = FxHashSet::default();
        seen_roots.insert(to_absolute_path_string(&host_module.src_path));

        for module_cfg in Self::router_map_module_order(config, host_module) {
            let root = to_absolute_path_string(&module_cfg.src_path);
            if !seen_roots.insert(root) {
                continue;
            }
            modules.push(RouterMapModule {
                module_name: module_cfg.name.clone(),
                package_name: Self::dependency_package_name(&module_cfg.src_path),
                src_path: module_cfg.src_path.clone(),
            });
        }

        for node in graph.source_hars() {
            if node.pkg_root.as_os_str().is_empty() {
                continue;
            }
            let root = to_absolute_path_string(&node.pkg_root);
            if !seen_roots.insert(root) {
                continue;
            }
            let module_name = Self::read_dependency_module_json(&node.pkg_root)
                .and_then(|module_json| module_json.module.name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| {
                    ModuleSpecifierText::new(&node.package_name)
                        .package_module_name_candidate()
                        .unwrap_or(&node.package_name)
                        .to_string()
                });
            modules.push(RouterMapModule {
                module_name,
                package_name: node.package_name.clone(),
                src_path: node.pkg_root.clone(),
            });
        }

        modules
    }

    pub(super) fn router_map_module_order<'a>(
        config: &'a ArkdownConfig,
        host_module: &ModuleConfig,
    ) -> Vec<&'a ModuleConfig> {
        let candidates = config
            .modules
            .iter()
            .filter(|module_cfg| module_cfg.name != host_module.name)
            .map(|module_cfg| {
                (
                    module_cfg,
                    Self::dependency_package_name(&module_cfg.src_path),
                )
            })
            .collect::<Vec<_>>();
        let dependencies = Self::read_dependency_package_json(&host_module.src_path)
            .map(|package| package.dependencies)
            .unwrap_or_default();

        let mut ordered = Vec::new();
        let mut seen = FxHashSet::default();
        for dep_name in dependencies.keys() {
            let Some((module_cfg, _)) = candidates.iter().find(|(module_cfg, package_name)| {
                !seen.contains(module_cfg.name.as_str())
                    && Self::dependency_matches_module(dep_name, module_cfg, package_name)
            }) else {
                continue;
            };
            seen.insert(module_cfg.name.clone());
            ordered.push(*module_cfg);
        }
        for (module_cfg, _) in candidates {
            if seen.insert(module_cfg.name.clone()) {
                ordered.push(module_cfg);
            }
        }
        ordered
    }

    pub(super) fn dependency_matches_module(
        dep_name: &str,
        module_cfg: &ModuleConfig,
        package_name: &str,
    ) -> bool {
        dep_name == package_name
            || dep_name == module_cfg.name
            || ModuleSpecifierText::new(dep_name)
                .package_module_name_candidate()
                .is_some_and(|name| name == package_name || name == module_cfg.name)
    }

    pub(super) fn read_loader_router_map(
        bundle_name: &str,
        host_module_name: &str,
        module_cfg: &RouterMapModule,
        router_map_ref: Option<&str>,
        pkg_context: Option<&PkgContextInfoOpt>,
    ) -> Vec<serde_json::Value> {
        let Some(profile_name) = router_map_ref.and_then(|value| value.strip_prefix("$profile:"))
        else {
            return vec![];
        };

        let module_layout = ProfileModuleLayout::new(&module_cfg.src_path);
        let path = module_layout.profile_json(profile_name);
        let Ok(content) = std::fs::read_to_string(path) else {
            return vec![];
        };
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&content) else {
            return vec![];
        };

        let Some(entries) = value
            .get_mut("routerMap")
            .and_then(serde_json::Value::as_array_mut)
        else {
            return value.as_array().cloned().unwrap_or_default();
        };

        for entry in entries.iter_mut() {
            let Some(obj) = entry.as_object_mut() else {
                continue;
            };
            let Some(page_source_file) = obj
                .get("pageSourceFile")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            if Path::new(&page_source_file).has_root() {
                continue;
            }
            let absolute = to_absolute_path_string(
                &module_layout.source_from_src_main_entry(Path::new(&page_source_file)),
            );
            let page_source_path = Self::source_entry_stem_path(&page_source_file)
                .and_then(|path| module_layout.source_entry_relative_to_ets(&path));
            let Some(rel_ets) = page_source_path else {
                continue;
            };
            let rel_ets = ArtifactPathSerializer::rel_to_slash(&Path::new("ets").join(rel_ets));
            let ohmurl = if let Some(context) = pkg_context {
                Self::normalized_ohm_from_pkg_context_source(context, &page_source_file)
            } else {
                let ohm_module_name =
                    Self::router_ohm_module_name(host_module_name, &module_cfg.module_name)
                        .string();
                Self::bundle_ohm_url(bundle_name, &ohm_module_name, Path::new(&rel_ets))
            };
            obj.insert("ohmurl".to_string(), serde_json::Value::String(ohmurl));
            obj.insert(
                "pageSourceFile".to_string(),
                serde_json::Value::String(absolute),
            );
        }

        entries.clone()
    }

    pub(super) fn read_module_router_map_ref(module_root: &Path) -> Option<String> {
        #[derive(serde::Deserialize)]
        struct ModuleJson {
            module: ModuleRouterMap,
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleRouterMap {
            router_map: Option<String>,
        }
        let layout = ProfileModuleLayout::new(module_root);
        for path in [layout.module_json5(), layout.module_json()] {
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            if let Ok(parsed) = json5::from_str::<ModuleJson>(&content) {
                if parsed.module.router_map.is_some() {
                    return parsed.module.router_map;
                }
            }
        }
        None
    }

    pub(super) fn collect_dependency_router_map_source_files(
        config: &ArkdownConfig,
        host_module: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
        pkg_contexts: Option<&std::collections::BTreeMap<String, PkgContextInfoOpt>>,
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for router_module in Self::dependency_router_map_modules(config, host_module, graph) {
            let Some(router_map_ref) = Self::read_module_router_map_ref(&router_module.src_path)
            else {
                continue;
            };
            let router_map = Self::read_loader_router_map(
                &config.product.bundle_name,
                &host_module.name,
                &router_module,
                Some(&router_map_ref),
                pkg_contexts.and_then(|contexts| contexts.get(&router_module.package_name)),
            );
            files.extend(router_map.iter().filter_map(|entry| {
                entry
                    .get("pageSourceFile")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
            }));
        }
        files
    }

    pub(super) fn collect_dependency_ability_source_files(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<PathBuf> {
        Self::collect_dependency_source_har_roots(graph)
            .into_iter()
            .flat_map(|dep_root| Self::collect_dependency_module_ability_source_files(&dep_root))
            .collect()
    }

    pub(super) fn collect_dependency_worker_files(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut seen = FxHashSet::default();
        for dep_root in Self::collect_dependency_source_har_roots(graph) {
            for worker in Self::collect_dependency_worker_entries(&dep_root) {
                let path = dep_root.join(arkdown_path::strip_relative_prefix(&worker));
                if Self::is_existing_loader_code_file(&path)
                    && seen.insert(to_absolute_path_string(&path))
                {
                    files.push(path);
                }
            }
        }
        files
    }

    pub(super) fn collect_dependency_worker_entries(dep_root: &Path) -> Vec<String> {
        let mut workers = Vec::new();
        if let Some(module_json) = Self::read_dependency_module_json(dep_root) {
            workers.extend(
                module_json
                    .module
                    .extension_abilities
                    .iter()
                    .filter_map(|ability| {
                        (ability.extension_type.as_deref() == Some("workScheduler"))
                            .then(|| ability.src_entry.clone())
                            .flatten()
                    }),
            );
        }
        workers.extend(Self::read_dependency_package_workers(dep_root));
        workers.extend(Self::read_dependency_build_profile_workers(dep_root));

        let mut seen = FxHashSet::default();
        workers.retain(|worker| seen.insert(worker.clone()));
        workers
    }

    pub(super) fn read_dependency_package_workers(dep_root: &Path) -> Vec<String> {
        let path = dep_root.join("oh-package.json5");
        let Ok(content) = std::fs::read_to_string(path) else {
            return vec![];
        };
        #[derive(Deserialize)]
        struct PackageWorkers {
            #[serde(default)]
            metadata: PackageWorkerMetadata,
        }
        #[derive(Default, Deserialize)]
        struct PackageWorkerMetadata {
            #[serde(default)]
            workers: Vec<String>,
        }
        json5::from_str::<PackageWorkers>(&content)
            .map(|package| package.metadata.workers)
            .unwrap_or_default()
    }

    pub(super) fn read_dependency_build_profile_workers(dep_root: &Path) -> Vec<String> {
        let path = dep_root.join("build-profile.json5");
        let Ok(content) = std::fs::read_to_string(path) else {
            return vec![];
        };
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildProfileWorkers {
            #[serde(default)]
            build_option: BuildProfileOption,
        }
        #[derive(Default, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildProfileOption {
            #[serde(default)]
            source_option: BuildProfileSourceOption,
        }
        #[derive(Default, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildProfileSourceOption {
            #[serde(default)]
            workers: Vec<String>,
        }
        json5::from_str::<BuildProfileWorkers>(&content)
            .map(|profile| profile.build_option.source_option.workers)
            .unwrap_or_default()
    }

    pub(super) fn collect_dependency_insight_intent_src_entries(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<PathBuf> {
        Self::collect_dependency_source_har_roots(graph)
            .into_iter()
            .flat_map(|dep_root| {
                Self::collect_dependency_module_insight_intent_src_entries(&dep_root)
            })
            .collect()
    }

    pub(super) fn collect_dependency_source_har_roots(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Vec<PathBuf> {
        graph
            .source_hars()
            .into_iter()
            .filter(|node| !node.pkg_root.as_os_str().is_empty())
            .map(|node| node.pkg_root.clone())
            .collect()
    }

    pub(super) fn collect_dependency_module_ability_source_files(dep_root: &Path) -> Vec<PathBuf> {
        let Some(module_json) = Self::read_dependency_module_json(dep_root) else {
            return vec![];
        };

        let src_main = ProfileModuleLayout::new(dep_root).src_main();
        module_json
            .module
            .abilities
            .iter()
            .chain(module_json.module.extension_abilities.iter())
            .filter_map(|ability| ability.src_entry.as_deref())
            .map(|src_entry| src_main.join(arkdown_path::strip_relative_prefix(src_entry)))
            .filter(|path| Self::is_existing_loader_code_file(path))
            .collect()
    }

    pub(super) fn collect_dependency_module_insight_intent_src_entries(
        dep_root: &Path,
    ) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        for resource_dir in Self::dependency_resource_dirs(dep_root) {
            let profile_path = resource_dir
                .join("base")
                .join("profile")
                .join("insight_intent.json");
            let Ok(content) = std::fs::read_to_string(profile_path) else {
                continue;
            };

            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct InsightIntentProfile {
                #[serde(default)]
                insight_intents_src_entry: Vec<InsightIntentEntry>,
            }

            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct InsightIntentEntry {
                src_entry: Option<String>,
            }

            let Ok(profile) = json5::from_str::<InsightIntentProfile>(&content) else {
                continue;
            };
            let src_main = ProfileModuleLayout::new(dep_root).src_main();
            entries.extend(
                profile
                    .insight_intents_src_entry
                    .iter()
                    .filter_map(|entry| entry.src_entry.as_deref())
                    .map(|src_entry| src_main.join(arkdown_path::strip_relative_prefix(src_entry)))
                    .filter(|path| Self::is_existing_loader_code_file(path)),
            );
        }
        entries
    }

    pub(super) fn dependency_resource_dirs(dep_root: &Path) -> Vec<PathBuf> {
        let directories = Self::read_dependency_package_json(dep_root)
            .and_then(|pkg| pkg.metadata)
            .and_then(|metadata| metadata.resource)
            .map(|resource| resource.directories)
            .unwrap_or_default();

        if directories.is_empty() {
            return vec![ProfileModuleLayout::new(dep_root).resources_root()];
        }

        directories
            .into_iter()
            .map(|directory| dep_root.join(arkdown_path::strip_relative_prefix(&directory)))
            .collect()
    }

    pub(super) fn read_dependency_package_json(pkg_root: &Path) -> Option<DependencyPackageJson> {
        let content = std::fs::read_to_string(pkg_root.join("oh-package.json5")).ok()?;
        json5::from_str::<DependencyPackageJson>(&content).ok()
    }

    pub(super) fn read_dependency_module_json(pkg_root: &Path) -> Option<DependencyModuleJson> {
        let layout = ProfileModuleLayout::new(pkg_root);
        [layout.module_json5(), layout.module_json()]
            .into_iter()
            .find_map(|path| std::fs::read_to_string(path).ok())
            .and_then(|content| json5::from_str::<DependencyModuleJson>(&content).ok())
    }

    pub(super) fn loader_build_config_path(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        target_name: &str,
    ) -> PathBuf {
        let layout = match config.output_target {
            OutputTarget::Preview => ProfileModuleLayout::for_preview_product(
                &module_cfg.src_path,
                &config.product.name,
                target_name,
            ),
            OutputTarget::Test => {
                ProfileModuleLayout::for_test_product(&module_cfg.src_path, &config.product.name)
            }
            _ if target_name != "default" => ProfileModuleLayout::for_target_product(
                &module_cfg.src_path,
                &config.product.name,
                target_name,
            ),
            _ => ProfileModuleLayout::for_product(&module_cfg.src_path, &config.product.name),
        };
        let candidates = layout.build_config_candidates();
        candidates
            .iter()
            .find(|path| path.is_file())
            .cloned()
            .unwrap_or_else(|| candidates[0].clone())
    }

    pub(super) fn read_loader_build_config(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content)
            .or_else(|_| json5::from_str(&content))
            .ok()
    }

    pub(super) fn collect_insight_intent_src_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let module_layout = ProfileModuleLayout::new(&module_cfg.src_path);
        let profile_path = module_layout.insight_intent_profile_json();
        let Ok(content) = std::fs::read_to_string(&profile_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InsightIntentProfile {
            #[serde(default)]
            insight_intents: Vec<InsightIntentEntry>,
            #[serde(default)]
            insight_intents_src_entry: Vec<InsightIntentEntry>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InsightIntentEntry {
            src_entry: Option<String>,
        }

        let Ok(profile) = json5::from_str::<InsightIntentProfile>(&content) else {
            return vec![];
        };
        let src_main = module_layout.src_main();
        let mut entries = Vec::new();
        for entry in profile
            .insight_intents
            .iter()
            .chain(profile.insight_intents_src_entry.iter())
        {
            let Some(src_entry) = entry.src_entry.as_deref() else {
                continue;
            };
            let path = src_main.join(arkdown_path::strip_relative_prefix(src_entry));
            if Self::is_existing_loader_code_file(&path) {
                entries.push(path);
            }
        }
        entries
    }

    pub(super) fn collect_mock_compile_entries(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        target_name: &str,
    ) -> Vec<PathBuf> {
        if !matches!(
            config.output_target,
            OutputTarget::Preview | OutputTarget::Test
        ) && target_name != "ohosTest"
        {
            return vec![];
        }

        let mock_config_path = ProfileModuleLayout::new(&module_cfg.src_path).mock_config_json5();
        let Ok(content) = std::fs::read_to_string(mock_config_path) else {
            return vec![];
        };
        let Ok(mock_entries) = json5::from_str::<IndexMap<String, serde_json::Value>>(&content)
        else {
            return vec![];
        };

        mock_entries
            .into_values()
            .filter_map(|entry| {
                let source = entry.get("source").and_then(|source| source.as_str())?;
                if source.is_empty() {
                    return None;
                }
                let path = module_cfg.src_path.join(source);
                Self::is_existing_hvigor_compile_entry_path(&path).then_some(path)
            })
            .collect()
    }

    /// Collect `src/ohosTest/ets/` source files for ohosTest-target builds.
    ///
    /// NOTE: the local unit-test build (`--target test`, `hvigorw test`) does
    /// NOT include `src/ohosTest` sources — its compiler inputs are the
    /// generated `.test/testability` templates plus `src/test` (see the
    /// local-test entry extension above). `src/ohosTest` participates in the
    /// separate `ohosTest` target build (`-p module=entry@ohosTest`), which is
    /// is selected by the module target name rather than `OutputTarget`.
    pub(super) fn collect_ohos_test_compile_entries(
        module_cfg: &ModuleConfig,
        target_name: &str,
    ) -> Vec<PathBuf> {
        if target_name != "ohosTest" {
            return vec![];
        }

        let test_ets_root = ProfileModuleLayout::new(&module_cfg.src_path).ohos_test_ets_root();
        if !test_ets_root.is_dir() {
            return vec![];
        }

        Self::collect_code_files(&test_ets_root)
    }

    pub(super) fn collect_code_files(root: &Path) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        let mut directories = std::collections::VecDeque::from([root.to_path_buf()]);
        while let Some(dir) = directories.pop_front() {
            let Ok(read_dir) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut dir_entries: Vec<_> = read_dir.flatten().collect();
            dir_entries.sort_by_key(|e| e.file_name());
            for entry in dir_entries {
                let path = entry.path();
                if path.is_dir() {
                    directories.push_back(path);
                } else if Self::is_existing_loader_code_file(&path) {
                    entries.push(path);
                }
            }
        }
        entries
    }

    pub(super) fn collect_app_startup_compile_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        entries.extend(Self::collect_app_startup_config_entry(module_cfg));
        entries.extend(Self::collect_app_startup_task_entries(module_cfg));
        entries
    }

    pub(super) fn collect_app_startup_config_entry(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let Some(app_startup_ref) = Self::read_module_app_startup_ref(module_cfg) else {
            return vec![];
        };
        let Some(profile_name) = app_startup_ref.strip_prefix("$profile:") else {
            return vec![];
        };
        let module_layout = ProfileModuleLayout::new(&module_cfg.src_path);
        let profile_path = module_layout.profile_json(profile_name);
        let Ok(content) = std::fs::read_to_string(profile_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct StartupProfile {
            config_entry: Option<String>,
        }

        let Ok(profile) = json5::from_str::<StartupProfile>(&content) else {
            return vec![];
        };
        let Some(config_entry) = profile.config_entry else {
            return vec![];
        };
        let path = module_layout.source_from_module_src(Path::new(
            arkdown_path::strip_relative_prefix(&config_entry),
        ));
        Self::is_existing_loader_code_file(&path)
            .then_some(path)
            .into_iter()
            .collect()
    }

    pub(super) fn collect_app_startup_task_entries(module_cfg: &ModuleConfig) -> Vec<PathBuf> {
        let temp_startup_path =
            ProfileModuleLayout::new(&module_cfg.src_path).startup_config_temp_json();
        let Ok(content) = std::fs::read_to_string(temp_startup_path) else {
            return vec![];
        };

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TempStartupConfig {
            #[serde(default)]
            startup_tasks: Vec<TempStartupTask>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TempStartupTask {
            src_entry_absolute_path: Option<PathBuf>,
        }

        let Ok(config) = serde_json::from_str::<TempStartupConfig>(&content)
            .or_else(|_| json5::from_str::<TempStartupConfig>(&content))
        else {
            return vec![];
        };
        config
            .startup_tasks
            .into_iter()
            .filter_map(|task| task.src_entry_absolute_path)
            .filter(|path| Self::is_existing_loader_code_file(path))
            .collect()
    }

    pub(super) fn read_module_app_startup_ref(module_cfg: &ModuleConfig) -> Option<String> {
        let path = ProfileModuleLayout::new(&module_cfg.src_path).module_json5();
        let content = std::fs::read_to_string(path).ok()?;
        #[derive(Deserialize)]
        struct ModuleJson {
            module: ModuleAppStartup,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ModuleAppStartup {
            app_startup: Option<String>,
        }
        json5::from_str::<ModuleJson>(&content)
            .ok()?
            .module
            .app_startup
    }

    pub(super) fn is_existing_loader_code_file(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            return false;
        };
        matches!(extension.to_ascii_lowercase().as_str(), "ets" | "ts" | "js")
            && !SourceFileName::new(path).is_declaration()
    }

    pub(super) fn is_existing_hvigor_compile_entry_path(path: &Path) -> bool {
        path.exists() && !SourceFileName::new(path).is_declaration()
    }

    /// Walk declared `oh_modules` dependencies, find installed HSP packages,
    /// and collect their `ResourceTable.txt` paths.
    ///
    /// Uses `LocalDependencyGraph` for BFS-consistent node ordering and
    /// correct type classification (only SharedHsp nodes contribute).
    /// Matches hvigor's `getHspResourcesMap()` for HSP dependencies.
    pub(super) fn collect_hsp_resources_map(
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> FxHashMap<String, String> {
        let mut map = FxHashMap::default();

        for node in graph.shared_hsps() {
            if node.pkg_root.as_os_str().is_empty() {
                continue; // RemoteOrUnknown with empty path
            }
            // Check for ResourceTable.txt in priority order:
            // 1. Build output intermediates (hvigor target intermediates layout)
            // 2. Dep root (remote HSP / pre-packaged layout)
            let resource_table = [
                ProfileModuleLayout::new(&node.pkg_root).resource_table_txt(),
                node.pkg_root.join("ResourceTable.txt"),
            ]
            .into_iter()
            .find(|p| p.is_file());
            if let Some(resource_table) = resource_table {
                map.entry(node.package_name.clone())
                    .or_insert_with(|| to_absolute_path_string(&resource_table));
            }
        }
        map
    }

    pub(super) fn dependency_package_name(pkg_dir: &Path) -> String {
        Self::read_dependency_package_json(pkg_dir)
            .and_then(|pkg| pkg.name)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| Self::dependency_package_name_from_path(pkg_dir))
    }

    pub(super) fn dependency_package_name_from_path(pkg_dir: &Path) -> String {
        let name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let parent = pkg_dir
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str());
        if let Some(parent) = parent.filter(|parent| {
            matches!(
                ModuleSpecifierText::new(parent).kind(),
                SpecifierKind::ScopedPackage
            )
        }) {
            ArtifactPath::from_slash(parent).join(name).into_string()
        } else {
            name.to_string()
        }
    }

    /// Scan HSP dependencies, and HAR dependencies only for ByteCode HAR
    /// modules, to build `hspNameOhmMap` / `harNameOhmMap`.
    ///
    /// Matches hvigor's `getHspNameOhmMap()` / `getHarNameOhmMap()`:
    /// `getHspNameOhmMap()` always collects HSP dependencies, while
    /// `getHarNameOhmMap()` returns `{}` unless `targetService.isByteCodeHar()`.
    /// In normalized mode both maps are produced from `pkgContextInfo` via
    /// `generateOhmUrl(...)`, so HAR dependencies keep an empty `moduleName`.
    ///
    /// Key:   dependency package name
    /// Value: OHM URL in the format determined by `useNormalizedOHMUrl`:
    ///   - `false` (default): `@bundle:<bundleName>/<moduleName>/<entryRelPath>`
    ///   - `true`:  `@normalized:N&<pkgContext.moduleName>&<pkgContext.bundleName>&<packageName>/<entryRelPath>&<pkgContext.version>`
    pub(super) fn collect_hsp_har_ohm_maps(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
        module_is_bytecode_har: bool,
    ) -> (IndexMap<String, String>, IndexMap<String, String>) {
        use crate::dependency_graph::DependencyNodeType;
        let bundle_name = &config.product.bundle_name;
        let use_normalized = config
            .product
            .build_option
            .strict_mode
            .use_normalized_ohm_url;

        let mut hsp_map = IndexMap::new();
        let mut har_map = IndexMap::new();
        let pkg_contexts = use_normalized
            .then(|| Self::dependency_pkg_contexts(config, module_cfg, graph))
            .flatten();

        for node in graph.nodes.values() {
            let is_hsp = node.node_type == DependencyNodeType::SharedHsp;
            let is_har_map_dependency = match node.node_type {
                // A bundled ByteCode HAR compiles source HARs into its ABC and
                // maps the remaining ByteCode HAR / OTHER barriers. The
                // coordinator replaces this coarse local-graph selection with
                // the exact session-tree view before serialization.
                DependencyNodeType::SourceHar => !config
                    .product
                    .build_option
                    .ark_options
                    .as_ref()
                    .is_some_and(|options| options.bundled_dependencies),
                DependencyNodeType::BytecodeHar
                | DependencyNodeType::NativeSo
                | DependencyNodeType::RuntimeOnly
                | DependencyNodeType::RemoteOrUnknown => true,
                DependencyNodeType::SharedHsp => false,
            };
            if !is_hsp && !(module_is_bytecode_har && use_normalized && is_har_map_dependency) {
                continue;
            }
            if node.pkg_root.as_os_str().is_empty() {
                continue;
            }

            let pkg_dir = &node.pkg_root;
            let Some(parsed) = Self::read_dependency_module_json(pkg_dir) else {
                continue;
            };
            let dep_name = &node.package_name;
            let entry_rel = Self::dep_entry_rel_path(pkg_dir);
            let ohm_url = if use_normalized {
                let Some(context) = pkg_contexts
                    .as_ref()
                    .and_then(|contexts| contexts.get(dep_name))
                else {
                    continue;
                };
                Self::normalized_ohm_from_pkg_context(context)
            } else {
                if is_har_map_dependency {
                    continue;
                }
                let module_name = parsed
                    .module
                    .name
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| {
                        ModuleSpecifierText::new(dep_name)
                            .package_module_name_candidate()
                            .unwrap_or(dep_name)
                            .to_string()
                    });
                OhmUrl::bundle(bundle_name, &module_name, &entry_rel).into_string()
            };

            if is_hsp {
                hsp_map.insert(dep_name.clone(), ohm_url);
            } else {
                har_map.insert(dep_name.clone(), ohm_url);
            }
        }

        (hsp_map, har_map)
    }

    pub(super) fn dependency_pkg_contexts(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> Option<std::collections::BTreeMap<String, PkgContextInfoOpt>> {
        use crate::dependency_graph::DependencyNodeType;
        let current_pkg = Self::read_dependency_package_json(&module_cfg.src_path);
        let current = PkgContextCurrentInput {
            package_name: current_pkg
                .as_ref()
                .and_then(|pkg| pkg.name.clone())
                .unwrap_or_else(|| module_cfg.name.clone()),
            bundle_name: config.product.bundle_name.clone(),
            bundle_type: "app".to_string(),
            module_kind: Self::pkg_context_module_kind(&module_cfg.module_type),
            module_root: module_cfg.src_path.clone(),
            source_root: ProfileModuleLayout::new(&module_cfg.src_path).src_main(),
            main: current_pkg.as_ref().and_then(|pkg| pkg.main.clone()),
            version: current_pkg
                .as_ref()
                .map(|pkg| pkg.version.clone())
                .unwrap_or_default(),
            oh_export_files: None,
        };
        let mut dependencies = Vec::new();
        for node in graph.nodes.values() {
            let kind = match node.node_type {
                DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar => {
                    PkgContextDependencyKind::Har
                }
                DependencyNodeType::SharedHsp => PkgContextDependencyKind::Hsp,
                DependencyNodeType::NativeSo => PkgContextDependencyKind::So,
                DependencyNodeType::RuntimeOnly | DependencyNodeType::RemoteOrUnknown => {
                    PkgContextDependencyKind::Other
                }
            };
            if node.pkg_root.as_os_str().is_empty() {
                continue;
            }
            let package = Self::read_dependency_package_json(&node.pkg_root);
            let module = Self::read_dependency_module_json(&node.pkg_root);
            dependencies.push(PkgContextDependencyInput {
                alias: node.package_name.clone(),
                package_name: node.package_name.clone(),
                version_spec: package.as_ref().map(|pkg| pkg.version.clone()),
                package_root: Some(node.pkg_root.clone()),
                node_entry: package
                    .as_ref()
                    .and_then(|pkg| pkg.main.clone())
                    .or_else(|| Some("index.js".to_string())),
                main: package
                    .as_ref()
                    .and_then(|pkg| pkg.main.clone().or_else(|| pkg.types.clone())),
                kind,
                module_name: module
                    .as_ref()
                    .and_then(|module| module.module.name.clone()),
                bundle_name: None,
                bundle_type: None,
                oh_export_files: None,
            });
        }
        let input = PkgContextInfoInput {
            current,
            dependencies,
            native_libraries: Vec::new(),
        };
        PkgContextInfoGenerator::new().generate(&input).ok()
    }

    pub(super) fn pkg_context_module_kind(module_type: &OhosModuleType) -> PkgContextModuleKind {
        match module_type {
            OhosModuleType::Shared => PkgContextModuleKind::Hsp,
            OhosModuleType::Har => PkgContextModuleKind::Har,
            OhosModuleType::Entry | OhosModuleType::Feature => PkgContextModuleKind::Hap,
        }
    }

    pub(super) fn normalized_ohm_from_pkg_context(context: &PkgContextInfoOpt) -> String {
        let normalized_import = if context.entry_path.is_empty() {
            String::new()
        } else {
            let entry_path = Self::source_entry_stem_path(&context.entry_path)
                .map(|path| ArtifactPathSerializer::rel_to_slash(&path))
                .unwrap_or_else(|| context.entry_path.clone());
            ArtifactPath::from_slash(&context.package_name)
                .join(&entry_path)
                .into_string()
        };
        OhmUrl::normalized_non_so(
            &context.module_name,
            &context.bundle_name,
            &normalized_import,
            &context.version,
        )
        .into_string()
    }

    pub(super) fn normalized_ohm_from_pkg_context_source(
        context: &PkgContextInfoOpt,
        source_entry: &str,
    ) -> String {
        let entry_path = Self::source_entry_stem_path(source_entry)
            .map(|path| ArtifactPathSerializer::rel_to_slash(&path))
            .unwrap_or_else(|| source_entry.to_string());
        let normalized_import = ArtifactPath::from_slash(&context.package_name)
            .join(&entry_path)
            .into_string();
        OhmUrl::normalized_non_so(
            &context.module_name,
            &context.bundle_name,
            &normalized_import,
            &context.version,
        )
        .into_string()
    }

    /// Return the entry file path relative to `pkg_dir` with extension stripped.
    ///
    /// Reads `oh-package.json5#main`; falls back to conventional candidates.
    pub(super) fn dep_entry_rel_path(pkg_dir: &Path) -> String {
        #[derive(Deserialize)]
        struct PkgEntry {
            #[serde(default)]
            main: Option<String>,
        }
        let main_file: Option<String> = std::fs::read_to_string(pkg_dir.join("oh-package.json5"))
            .ok()
            .and_then(|s| json5::from_str::<PkgEntry>(&s).ok())
            .and_then(|p| p.main)
            .filter(|m| !m.is_empty());

        if let Some(main) = main_file {
            if let Some(rel) = Self::source_entry_stem_path(&main) {
                return ArtifactPathSerializer::rel_to_slash(&rel);
            }
        }

        // Conventional fallbacks: these cover the common package layouts.
        for candidate_rel in &["src/main/ets/index", "src/main/ets/Index", "index", "Index"] {
            let candidate = pkg_dir.join(Path::new(candidate_rel).with_extension("ets"));
            if candidate.exists() {
                return candidate_rel.to_string();
            }
            let candidate = pkg_dir.join(Path::new(candidate_rel).with_extension("ts"));
            if candidate.exists() {
                return candidate_rel.to_string();
            }
        }

        "index".to_string()
    }

    pub(super) fn build_pkg_brief_info(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        graph: &crate::dependency_graph::LocalDependencyGraph,
    ) -> FxHashMap<String, PkgBriefInfo> {
        let mut map = FxHashMap::default();
        let module_root = to_absolute_path_string(&module_cfg.src_path);
        let ets_src = to_absolute_path_string(
            &ProfileModuleLayout::new(&module_cfg.src_path).ets_source_root(),
        );

        // Detect effective target to include target-specific source root
        // (`src/<target>/ets/`) when a non-default target is active.
        //
        // hvigor adds `src/<target>` as an additional `sourceRoot` in
        // `pkgNameToPkgBriefInfo` when the effective target is not `default`.
        // This allows the compiler to resolve target-specific source files.
        let build_mode_name = config.build_mode.as_str();
        let target_ets_src = Self::detect_target_specific_ets_src(
            &module_cfg.src_path,
            &config.project_root,
            &module_cfg.name,
            &config.product.name,
            build_mode_name,
        );

        // Add the current module with original (src/main/ets) as originalSourceRoots
        // and target-specific source root (when present) prepended to sourceRoots.
        let source_roots = if let Some(ref target_ets) = target_ets_src {
            vec![target_ets.clone(), ets_src.clone()]
        } else {
            vec![ets_src.clone()]
        };
        map.insert(
            module_cfg.name.clone(),
            PkgBriefInfo {
                pkg_root: module_root.clone(),
                pkg_name: module_cfg.name.clone(),
                source_roots,
                original_source_roots: vec![ets_src],
            },
        );

        // Add all other workspace modules (hvigor includes the full target module graph).
        for project_module in &config.modules {
            if project_module.name == module_cfg.name {
                continue; // already added above
            }
            let pkg_root = to_absolute_path_string(&project_module.src_path);
            let pkg_ets_src = to_absolute_path_string(
                &ProfileModuleLayout::new(&project_module.src_path).ets_source_root(),
            );
            map.insert(
                project_module.name.clone(),
                PkgBriefInfo {
                    pkg_root: pkg_root.clone(),
                    pkg_name: project_module.name.clone(),
                    source_roots: vec![pkg_ets_src.clone()],
                    original_source_roots: vec![pkg_ets_src],
                },
            );
        }

        // Add source-type dependency packages from the shared dependency graph.
        // BytecodeHar and NativeSo are excluded (no ETS sources to compile).
        {
            use crate::dependency_graph::DependencyNodeType;
            for node in graph.nodes.values() {
                if matches!(
                    node.node_type,
                    DependencyNodeType::BytecodeHar | DependencyNodeType::NativeSo
                ) {
                    continue; // pre-compiled or native-only: no ETS sources
                }
                if node.pkg_root.as_os_str().is_empty() {
                    continue; // remote package not installed locally
                }
                let ets_dir = ProfileModuleLayout::new(&node.pkg_root).ets_source_root();
                if !ets_dir.is_dir() {
                    continue; // no source tree → skip
                }
                let pkg_name = &node.package_name;
                if map.contains_key(pkg_name.as_str()) {
                    continue; // prefer workspace module entry
                }
                let pkg_root_str = to_absolute_path_string(&node.pkg_root);
                let pkg_ets_src = to_absolute_path_string(&ets_dir);
                map.insert(
                    pkg_name.clone(),
                    PkgBriefInfo {
                        pkg_root: pkg_root_str,
                        pkg_name: pkg_name.clone(),
                        source_roots: vec![pkg_ets_src.clone()],
                        original_source_roots: vec![pkg_ets_src],
                    },
                );
            }
        }

        map
    }

    /// Detect an active non-default target's `src/<target>/ets/` directory.
    ///
    /// hvigor includes this path in `pkgNameToPkgBriefInfo.sourceRoots` when
    /// a non-default effective target is selected.  The `originalSourceRoots`
    /// keeps `src/main/ets` so the compiler can resolve both roots.
    ///
    /// Returns `None` when:
    /// - There is no non-default effective target for this module.
    /// - The effective target's `src/<name>/ets/` directory does not exist.
    pub(super) fn detect_target_specific_ets_src(
        module_root: &std::path::Path,
        project_root: &std::path::Path,
        module_name: &str,
        product_name: &str,
        build_mode_name: &str,
    ) -> Option<String> {
        use arkdown_config::build_profile::RawBuildProfile;

        let profile = RawBuildProfile::load(project_root).ok()?;
        let raw_module = profile.modules.iter().find(|m| m.name == module_name)?;

        // Use centralised target selection for consistent priority rules.
        let target = arkdown_config::build_profile::pick_effective_raw_target(
            &raw_module.targets,
            product_name,
            build_mode_name,
        )?;

        // Only emit a target source root for non-default targets.
        if target.name == "default" {
            return None;
        }

        let target_ets = module_root.join("src").join(&target.name).join("ets");
        if target_ets.is_dir() {
            Some(to_absolute_path_string(&target_ets))
        } else {
            None
        }
    }
}
