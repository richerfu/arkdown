//! Loader.json generation and compile-entry construction.

use super::*;

struct RuntimeEntrySources {
    all: Vec<String>,
    pages: Vec<String>,
}

impl LoaderJson {
    /// Apply the validated current-package `oh-exports` file snapshot.
    ///
    /// Mirrors `GenerateLoaderJson.addOhExports2CompileEntry`,
    /// `processCodeFiles`, and `writeOhExportsToLoaderJson`: declaration files
    /// remain visible in `oh-exports`, but are not compile entries.
    pub(crate) fn apply_oh_exports_files(&mut self, files: &[PathBuf]) {
        self.oh_exports = files
            .iter()
            .map(|path| {
                ArtifactPathSerializer::path_to_slash_string(path)
                    .unwrap_or_else(|_| path.display().to_string())
            })
            .collect();

        let source_root = Path::new(&self.module_root_path);
        for path in files {
            let lower_path = path.to_string_lossy().to_ascii_lowercase();
            if lower_path.ends_with(".d.ets") || lower_path.ends_with(".d.ts") || !path.exists() {
                continue;
            }
            let absolute = to_absolute_path_string(path);
            if !self.hvigor_compile_entry.contains(&absolute) {
                self.hvigor_compile_entry.push(absolute.clone());
            }
            let canonical_path = canonicalize_or_owned(path);
            let internal_entry = canonical_path
                .strip_prefix(source_root)
                .map(Self::compile_entry_from_src_main_rel)
                .unwrap_or(absolute);
            if !self.compile_entry.contains(&internal_entry) {
                self.compile_entry.push(internal_entry);
            }
        }
    }

    /// Add the generated `ModuleInfo.ts` required by a ByteCode HAR.
    ///
    /// Mirrors `GenerateLoaderJson.processByteCodeHar()` from the installed
    /// hvigor 6.24.3. The path participates in both the serialized loader
    /// contract and Arkdown's in-process compiler input inventory.
    pub fn include_byte_code_har_module_info(&mut self, path: &Path) {
        if !path.is_file() {
            return;
        }
        let absolute = to_absolute_path_string(path);
        if !self.hvigor_compile_entry.contains(&absolute) {
            self.hvigor_compile_entry.push(absolute.clone());
        }
        if !self.compile_entry.contains(&absolute) {
            self.compile_entry.push(absolute);
        }
    }

    /// Apply the final ByteCode HAR scan result to this loader.
    ///
    /// Profile generation can run before the session-backed dependency graph is
    /// available. The coordinator later supplies the authoritative scan result,
    /// and all ByteCode HAR-derived loader surfaces must move together:
    /// `byteCodeHarInfo`, propagated `declarationEntry`, serialized
    /// `otherCompileFiles`, `updateVersionInfo`, and arkdown's internal
    /// compile entry list.
    pub fn apply_bytecode_har_scan_entries(
        &mut self,
        deps: &[ByteCodeHarScanEntry],
        graph: &arkdown_session::DependencyGraphSnapshot,
        output_target: OutputTarget,
    ) {
        let previous_other_compile_files = std::mem::take(&mut self.other_compile_files);
        self.compile_entry
            .retain(|entry| !previous_other_compile_files.contains(entry));

        // Installed hvigor 6.24.3
        // `GenerateLoaderJson.setByteCodeHarInfoAndOtherCompileFiles()` gates
        // this entire surface on normalized OHM URLs and `isHapOrHsp()`.
        // A normal HAR target therefore keeps these fields empty even when it
        // is itself emitted as a ByteCode HAR.
        if !self.use_normalized_ohm_url || matches!(output_target, OutputTarget::Har) {
            self.byte_code_har_info.clear();
            self.declaration_entry.clear();
            self.update_version_info.clear();
            return;
        }

        self.byte_code_har_info = deps
            .iter()
            .cloned()
            .map(ByteCodeHarScanEntry::into_loader_entry)
            .collect();
        self.update_version_info =
            ByteCodeHarScanEntry::collect_bytecode_har_update_version_info_from_graph(deps, graph);

        if matches!(
            output_target,
            OutputTarget::Hap | OutputTarget::Hsp | OutputTarget::Preview | OutputTarget::Test
        ) {
            let mut entries = deps
                .iter()
                .flat_map(|dep| dep.declaration_entry.iter())
                .filter(|url| !url.trim().is_empty())
                .cloned()
                .collect::<Vec<_>>();
            entries.sort();
            entries.dedup();
            self.declaration_entry = entries;
        }

        let other_compile_paths =
            LoaderByteCodeHarFields::collect_other_compile_files_from_graph(deps, graph);
        self.other_compile_files = other_compile_paths
            .iter()
            .map(|path| to_absolute_path_string(path))
            .collect();
        self.compile_entry
            .extend(self.other_compile_files.iter().cloned());
        let mut seen = FxHashSet::default();
        self.compile_entry
            .retain(|entry| seen.insert(entry.clone()));
    }

    /// Replace `harNameOhmMap` from the exact dependency view selected by
    /// hvigor's ByteCode HAR task service.
    pub fn apply_bytecode_har_ohm_packages(
        &mut self,
        package_names: &[String],
        pkg_context_info: &std::collections::BTreeMap<String, PkgContextInfoOpt>,
    ) {
        if self.byte_code_har != Some(true) || !self.use_normalized_ohm_url {
            return;
        }
        self.har_name_ohm_map.clear();
        for package_name in package_names {
            let Some(context) = pkg_context_info.get(package_name) else {
                continue;
            };
            self.har_name_ohm_map.insert(
                package_name.clone(),
                Self::normalized_ohm_from_pkg_context(context),
            );
        }
    }

    pub(super) fn compile_entry_from_src_main_rel(path: &Path) -> String {
        ModuleSpecifier::from_relative_path(path).into_string()
    }

    pub(super) fn compile_entry_from_ets_rel(path: &Path) -> String {
        let path = Path::new("ets").join(path);
        Self::compile_entry_from_src_main_rel(&path)
    }

    pub(super) fn compile_entry_from_page(page: &str) -> String {
        let mut page_path = PathBuf::from(arkdown_path::strip_relative_prefix(page));
        if !Self::page_path_is_src_main_relative(&page_path) {
            page_path = Path::new("ets").join(page_path);
        }
        let mut path = Self::source_path_without_extension(&page_path);
        path.set_extension("ets");
        Self::compile_entry_from_src_main_rel(&path)
    }

    pub(super) fn page_path_is_src_main_relative(path: &Path) -> bool {
        path.components().next().is_some_and(
            |component| matches!(component, Component::Normal(name) if name == OsStr::new("ets")),
        )
    }

    pub(super) fn absolute_dir_path_string(path: &Path) -> String {
        let mut serialized = to_absolute_path_string(path);
        if !serialized.ends_with('/') {
            serialized.push('/');
        }
        serialized
    }

    pub(super) fn source_entry_stem_path(source_entry: &str) -> Option<PathBuf> {
        let source_entry = arkdown_path::strip_relative_prefix(source_entry);
        let mut path = PathBuf::new();
        for component in Path::new(source_entry).components() {
            if matches!(component, Component::CurDir) {
                continue;
            }
            path.push(component.as_os_str());
        }
        (!path.as_os_str().is_empty()).then(|| Self::source_path_without_extension(&path))
    }

    pub(super) fn source_path_without_extension(path: &Path) -> PathBuf {
        let mut path = path.to_path_buf();
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts" | "js")
        ) {
            path.set_extension("");
        }
        path
    }

    fn collect_runtime_entry_sources(
        config: &ArkdownConfig,
        manifest: &HapManifest,
        module_layout: &ProfileModuleLayout<'_>,
        target_name: &str,
    ) -> Result<RuntimeEntrySources, ArkdownError> {
        let mut runtime_entry_sources: Vec<String> = manifest
            .abilities
            .iter()
            .map(|a| a.src_entry.clone())
            .chain(
                manifest
                    .extension_abilities
                    .iter()
                    .map(|ext| ext.src_entry.clone()),
            )
            .chain(manifest.src_entry.iter().cloned())
            .collect();

        // Local unit-test builds replace the module pages with the generated
        // `.test/testability/pages/Index` entry; the real main pages do not
        // participate in the `test` compilation (hvigor `UnitTestArkTS`
        // inputs). The `ohosTest` target similarly compiles the generated
        // `src/ohosTest` pages (`$profile:test_pages` lives in the template
        // tree, not `src/main/resources`). Preview keeps the normal pages.
        let page_entry_sources = if let Some(pages_ref) = &manifest.pages {
            if !matches!(config.output_target, OutputTarget::Test) && target_name != "ohosTest" {
                Self::read_main_page_entries(module_layout, pages_ref)?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        runtime_entry_sources.extend(page_entry_sources.iter().cloned());

        Ok(RuntimeEntrySources {
            all: runtime_entry_sources,
            pages: page_entry_sources,
        })
    }

    pub(super) fn read_main_page_entries(
        module_layout: &ProfileModuleLayout<'_>,
        pages_ref: &str,
    ) -> Result<Vec<String>, ArkdownError> {
        let profile_name =
            ProfileResourceRef::profile_name_or_bare(pages_ref).ok_or_else(|| {
                ArkdownError::ParseError {
                    file: module_layout.module_json5().display().to_string(),
                    message: "module.pages must reference a profile resource".to_string(),
                }
            })?;
        let profile_path = module_layout.profile_json(profile_name);
        let content = std::fs::read_to_string(&profile_path)?;
        let pages = serde_json::from_str::<MainPagesProfile>(&content).map_err(|error| {
            ArkdownError::ParseError {
                file: profile_path.display().to_string(),
                message: error.to_string(),
            }
        })?;

        Ok(pages
            .src
            .iter()
            .map(|page| Self::compile_entry_from_page(page))
            .collect())
    }

    pub(super) fn bundle_ohm_url(
        bundle_name: &str,
        module_name: &str,
        source_path: &Path,
    ) -> String {
        OhmUrl::bundle(
            bundle_name,
            module_name,
            &ArtifactPathSerializer::rel_to_slash(source_path),
        )
        .into_string()
    }

    pub(super) fn router_ohm_module_name<'a>(
        host_module_name: &'a str,
        module_name: &'a str,
    ) -> LoaderRouterModuleName<'a> {
        LoaderRouterModuleName::new(host_module_name, module_name)
    }

    /// Build a [`LoaderJson`] from the processed profile data.
    ///
    /// Mirrors hvigor's `GenerateLoaderJson` task, which produces the manifest
    /// the bundler reads for entry points, module paths, and dependency metadata.
    pub fn generate(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
    ) -> Self {
        Self::try_generate(config, module_cfg, manifest, "default")
            .unwrap_or_else(|err| panic!("failed to generate loader.json: {err}"))
    }

    /// Fallible form of [`LoaderJson::generate`] with an explicit module
    /// target name (e.g. `ohosTest` keys `generated/r/<target>/`).
    pub fn try_generate(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
        target_name: &str,
    ) -> Result<Self, ArkdownError> {
        // Standalone callers still get the same immutable graph type used by
        // the build session. The production coordinator supplies its already
        // frozen graph through `try_generate_with_dependency_graph`.
        let dep_graph = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dependency_graph_snapshot");
            arkdown_session::dependency_graph::builder::DependencyGraphBuilder::new(
                &module_cfg.name,
                &config.project_root,
                &module_cfg.src_path,
            )
            .with_runtime_only_aliases(
                config
                    .product
                    .build_option
                    .ark_options
                    .as_ref()
                    .and_then(|options| options.runtime_only.as_ref())
                    .into_iter()
                    .flat_map(|runtime_only| runtime_only.packages.iter().cloned()),
            )
            .build()
        };
        Self::try_generate_with_dependency_graph(
            config,
            module_cfg,
            manifest,
            &dep_graph,
            target_name,
            LoaderCompilationPolicy::default(),
        )
    }

    pub(crate) fn try_generate_with_dependency_graph(
        config: &ArkdownConfig,
        module_cfg: &ModuleConfig,
        manifest: &HapManifest,
        dep_graph: &arkdown_session::DependencyGraphSnapshot,
        target_name: &str,
        compilation_policy: LoaderCompilationPolicy,
    ) -> Result<Self, ArkdownError> {
        // Local unit-test builds root at `<module>/.test` (hvigor
        // `buildRoot=.test`), which moves cache/assets/loader paths below it.
        let module_layout = if matches!(config.output_target, OutputTarget::Test) {
            ProfileModuleLayout::for_test_product(&module_cfg.src_path, &config.product.name)
        } else if matches!(config.output_target, OutputTarget::Preview) {
            ProfileModuleLayout::for_preview_product(
                &module_cfg.src_path,
                &config.product.name,
                target_name,
            )
        } else if target_name != "default" {
            ProfileModuleLayout::for_target_product(
                &module_cfg.src_path,
                &config.product.name,
                target_name,
            )
        } else {
            ProfileModuleLayout::for_product(&module_cfg.src_path, &config.product.name)
        };
        let runtime_entries = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "runtime_entry_sources");
            Self::collect_runtime_entry_sources(config, manifest, &module_layout, target_name)?
        };
        let runtime_entry_sources = runtime_entries.all;
        let page_entry_sources = runtime_entries.pages;

        // Collect compile_entry from runtime srcEntry values.
        let mut compile_entry = Vec::new();
        if matches!(config.output_target, OutputTarget::Test) {
            // hvigor UnitTestArkTS input order puts the generated testability
            // templates before the module abilities (filesInfo.txt:
            // ResourceTable, TestAbility, Index, EntryAbility, List.test).
            // es2abc compiles in input order, so the entry order must match.
            let testability_dir = module_layout.testability_dir();
            for entry in [
                testability_dir.join("TestAbility.ets"),
                testability_dir.join("pages").join("Index.ets"),
            ] {
                if entry.is_file() {
                    compile_entry.push(to_absolute_path_string(&entry));
                }
            }
            // `UnitTestArkCompile.initDefaultArkCompileConfig()` supplies
            // `mainModulePath` for the first selected entry module. The SDK
            // compiler consequently adds that module's ability and extension
            // srcEntry values even though local-test loader.json serializes
            // only mock sources and ResourceTable.ts. Arkdown's in-process
            // bundler needs the same entries explicitly.
            compile_entry.extend(runtime_entry_sources.iter().cloned());
        } else if target_name == "ohosTest" {
            // The `ohosTest` target compiles the generated template tree
            // (`.test/<product>/intermediates/src/ohosTest/ets/`) — the
            // TestAbility srcEntry points there and the test pages live in
            // the template `resources`. Relative `./ets/...` srcEntry values
            // resolve against the module ets root, which does not exist for
            // the ohosTest module; the absolute template paths below are the
            // compiler inputs.
            let template_ets = module_layout.ohos_test_template_ets_root();
            for entry in [
                template_ets.join("testability").join("TestAbility.ets"),
                template_ets
                    .join("testability")
                    .join("pages")
                    .join("Index.ets"),
                template_ets
                    .join("testrunner")
                    .join("OpenHarmonyTestRunner.ets"),
            ] {
                if entry.is_file() {
                    compile_entry.push(to_absolute_path_string(&entry));
                }
            }
            for source in runtime_entry_sources
                .iter()
                .filter(|source| !Path::new(source).has_root() && !source.starts_with("./ets/"))
            {
                compile_entry.push(source.clone());
            }
        } else {
            compile_entry.extend(runtime_entry_sources.iter().cloned());
        }
        let runtime_only_collector = RuntimeOnlyCollector::new(config, module_cfg, dep_graph);
        let module_json_path = module_layout.module_json5();
        let form_sources = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "form_sources");
            FormProfileProcessor::new().collect_module_form_sources(ModuleFormProfileInput {
                module_name: &module_cfg.name,
                module_root: &module_cfg.src_path,
                module_type: &module_cfg.module_type,
                module_json_path: &module_json_path,
                extension_abilities: &manifest.extension_abilities,
                project_modules: &config.modules,
                compatible_api_version: parse_api_version(&config.product.compatible_sdk_version)
                    .or(Some(config.sdk_config.compile_api_version)),
            })?
        };
        let src_main_root = module_layout.src_main();
        // Widget (form card) sources never enter the module compile entries:
        // hvigor compiles them in the dedicated widget pass (`widgets.abc`)
        // under `needSubmitArkTsWidget`, while the non-full-compilation
        // `compileEntry` only carries ResourceTable.ts and the configured
        // entries. The form collection above still runs for its profile
        // validations (`FORM_WIDGET_MODULE_INVALID_*`).
        let form_entry_sources = form_sources
            .iter()
            .map(|form_source| {
                if let Ok(rel) = form_source.source_path.strip_prefix(&src_main_root) {
                    Self::compile_entry_from_src_main_rel(rel)
                } else {
                    to_absolute_path_string(&form_source.source_path)
                }
            })
            .collect::<Vec<_>>();
        let insight_intent_form_bindings = {
            let mut bindings = IndexMap::<String, InsightIntentFormBinding>::new();
            for form_source in &form_sources {
                if form_source.ability_src_entry.is_empty() {
                    continue;
                }
                let binding = bindings
                    .entry(form_source.ability_src_entry.clone())
                    .or_insert_with(|| InsightIntentFormBinding {
                        source: form_source.ability_src_entry.clone(),
                        ability_name: form_source.ability_name.clone(),
                        form_names: Vec::new(),
                    });
                if let Some(name) = form_source.name.as_ref() {
                    binding.form_names.push(name.clone());
                }
            }
            bindings
                .into_values()
                .map(|mut binding| {
                    binding.form_names.sort();
                    binding.form_names.dedup();
                    binding
                })
                .collect::<Vec<_>>()
        };

        // Module-level srcEntry (used by HSP shared packages that export a single file
        // without declaring abilities).
        if let Some(src_entry) = &manifest.src_entry {
            compile_entry.push(src_entry.clone());
        }
        let mut package_main_entry = None;
        if compile_entry.is_empty()
            && matches!(
                manifest.module_type,
                OhosModuleType::Har | OhosModuleType::Shared
            )
        {
            if let Some(main) = Self::read_package_main(&module_cfg.src_path) {
                let main = to_absolute_path_string(&module_cfg.src_path.join(main));
                package_main_entry = Some(main.clone());
                compile_entry.push(main);
            }
        }

        // Discover WebWorker files from src/main/ets/workers. Upstream
        // `main.js::readWorkerFile()` accepts explicit loader.json `workers` entries
        // with .ets/.ts/.js suffixes; `loadWorker()` otherwise scans this directory
        // recursively. Keep both surfaces aligned: serialize absolute worker paths
        // for loader consumers and add relative `./ets/...` entries for arkdown's
        // in-process bundler.
        let worker_files = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "worker_files");
            runtime_only_collector.discover_worker_files()
        };
        let mut discovered_workers: Vec<String> = vec![];
        let ets_root = module_layout.ets_source_root();
        for path in &worker_files {
            if let Ok(rel) = path.strip_prefix(&ets_root) {
                compile_entry.push(Self::compile_entry_from_ets_rel(rel));
            }
            discovered_workers.push(to_absolute_path_string(path));
        }
        let dependency_worker_files = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dependency_worker_files");
            Self::collect_dependency_worker_files(dep_graph)
        };
        for path in &dependency_worker_files {
            let abs = to_absolute_path_string(path);
            if !discovered_workers.contains(&abs) {
                discovered_workers.push(abs.clone());
            }
            // The SDK loader consumes `workers` as a separate compiler entry
            // surface. Arkdown's in-process bundler has one input list, so keep
            // every dependency worker reachable internally without leaking it
            // into serialized HAP/HSP `compileEntry` below.
            compile_entry.push(abs);
        }

        // Merge in explicit sourceOption.workers entries from build-profile.json5.
        // Hvigor's `targetService.getWorkerConfig()` resolves these to absolute paths;
        // we do the same in resolve.rs, so they arrive here already absolute.
        if let Some(source_opt) = config.product.build_option.source_option.as_ref() {
            for worker_path in &source_opt.workers {
                let abs = to_absolute_path_string(worker_path);
                if !discovered_workers.contains(&abs) {
                    // Also add a compile entry if the file is under src/main/ets.
                    if let Ok(rel) = worker_path.strip_prefix(&ets_root) {
                        let entry = Self::compile_entry_from_ets_rel(rel);
                        if !compile_entry.contains(&entry) {
                            compile_entry.push(entry);
                        }
                    }
                    discovered_workers.push(abs);
                }
            }
        }

        let use_normalized_ohm_url = config
            .product
            .build_option
            .strict_mode
            .use_normalized_ohm_url;
        let module_is_bytecode_har = matches!(config.output_target, OutputTarget::Har)
            && matches!(module_cfg.module_type, OhosModuleType::Har)
            && matches!(
                config.product.build_option.har_artifact_type,
                arkdown_core::HarArtifactType::Bytecode
            );
        let collects_bytecode_har_inputs =
            use_normalized_ohm_url && !matches!(config.output_target, OutputTarget::Har);
        let bundles_bytecode_har_dependencies = matches!(config.output_target, OutputTarget::Har)
            && matches!(
                config.product.build_option.har_artifact_type,
                arkdown_core::HarArtifactType::Bytecode
            )
            && config
                .product
                .build_option
                .ark_options
                .as_ref()
                .is_some_and(|options| options.bundled_dependencies);
        let pkg_contexts = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dependency_pkg_contexts");
            use_normalized_ohm_url
                .then(|| Self::dependency_pkg_contexts(config, module_cfg, dep_graph))
                .flatten()
        };
        let router_map = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "router_map");
            Self::read_loader_router_maps(
                config,
                module_cfg,
                manifest.router_map.as_deref(),
                dep_graph,
                pkg_contexts.as_ref(),
            )
        };

        // Full HSP compilation starts from every code file below
        // `src/main/ets`. This mirrors GenerateLoaderJson's
        // allowToCollectEtsFolder()/collectCompileFiles contract and ensures
        // exported shared-library files participate even without abilities.
        // Form widget modules compile in non-full-compilation mode
        // (`isFullCompilationEnabled` is false without `OHOS_COMPILE_LIB_ENTRY`):
        // hvigor's compileEntry only carries ResourceTable.ts and the
        // configured entries, never the ets tree (widget pages go to
        // `widgets.abc` via `needSubmitArkTsWidget`).
        let is_form_widget_module = std::fs::read_to_string(&module_json_path)
            .ok()
            .and_then(|content| json5::from_str::<ModuleJson5>(&content).ok())
            .is_some_and(|json| json.module.form_extension_module.is_some());
        let full_compilation =
            compilation_policy.is_full_compilation_enabled(is_form_widget_module);
        let collect_own_ets = target_name != "ohosTest"
            && full_compilation
            && matches!(
                module_cfg.module_type,
                OhosModuleType::Shared | OhosModuleType::Har
            );
        let mut serialized_compile_entry = if collect_own_ets {
            Self::collect_code_files(&ets_root)
        } else {
            Vec::new()
        };
        // Dependency ETS folders are not unconditional full-compilation
        // inputs. GenerateLoaderJson.collectDepEtsFolderFilesEntry() is called
        // only from processHarDynamicImport() for a selected runtimeOnly HAR;
        // RuntimeOnlyCollector::collect_package_source_files() below owns that
        // filtered path, including remote-HAR exclusion.
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "insight_intent_compile_entries");
            Self::collect_insight_intent_src_entries(module_cfg)
        });
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "mock_compile_entries");
            Self::collect_mock_compile_entries(config, module_cfg, target_name)
        });
        // Device-test target: add the module's src/ohosTest/ets sources.
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "ohos_test_compile_entries");
            Self::collect_ohos_test_compile_entries(module_cfg, target_name)
        });
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "app_startup_compile_entries");
            Self::collect_app_startup_compile_entries(module_cfg)
        });
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "runtime_only_source_files");
            runtime_only_collector.collect_source_files()
        });
        serialized_compile_entry.extend({
            let _timer =
                LoaderTimingProbe::new(&module_cfg.name, "runtime_only_package_source_files");
            runtime_only_collector.collect_package_source_files()
        });
        // `GenerateLoaderJson.processBundledDependencies` adds dependency
        // workers only while producing a bundled ByteCode HAR. HAP/HSP builds
        // still expose them through `workers`, but compile dependency abilities
        // through `collectDepAbilitySrcEntryArr` below.
        if bundles_bytecode_har_dependencies {
            serialized_compile_entry.extend(dependency_worker_files);
        }
        serialized_compile_entry.extend({
            let _timer =
                LoaderTimingProbe::new(&module_cfg.name, "dependency_router_map_source_files");
            Self::collect_dependency_router_map_source_files(
                config,
                module_cfg,
                dep_graph,
                pkg_contexts.as_ref(),
            )
        });
        serialized_compile_entry.extend({
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dependency_ability_sources");
            Self::collect_dependency_ability_source_files(dep_graph)
        });
        serialized_compile_entry.extend({
            let _timer =
                LoaderTimingProbe::new(&module_cfg.name, "dependency_insight_intent_sources");
            Self::collect_dependency_insight_intent_src_entries(dep_graph)
        });
        {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "compile_entry_dedupe");
            runtime_only_collector.dedupe_paths(&mut serialized_compile_entry);
        }
        // Latest GenerateLoaderJson.addResourceTable2CompileEntry appends the
        // restool-generated TS table after normal source filtering. It gates
        // on the selected SDK component API, not the project's target API.
        if !matches!(module_cfg.module_type, OhosModuleType::Har)
            && !matches!(config.output_target, OutputTarget::Preview)
            && config.sdk_config.sdk_component_api_version >= 23
        {
            serialized_compile_entry.push(module_layout.generated_resource_table_typescript());
        }

        let mut hvigor_compile_entry: Vec<String> = serialized_compile_entry
            .iter()
            .map(|path| to_absolute_path_string(path))
            .collect();
        if matches!(config.output_target, OutputTarget::Test) {
            // Local unit-test builds compile in non-full-compilation mode:
            // hvigor's serialized compileEntry carries only ResourceTable.ts
            // (plus mock sources — `collectMockFileEntry` runs before
            // `addResourceTable2CompileEntry` for local-test mode). The
            // testability templates and src/test sources reach the compiler
            // through moduleJson abilities / the replaced main_pages entry.
            let mock_sources_dir = canonicalize_or_owned(&module_layout.mock_sources_dir());
            hvigor_compile_entry.retain(|entry| {
                let path = Path::new(entry);
                path.file_name() == Some(OsStr::new("ResourceTable.ts"))
                    || path.strip_prefix(&mock_sources_dir).is_ok()
            });
        } else if target_name == "ohosTest" {
            // GenerateLoaderJson does not serialize the target's
            // `src/ohosTest/ets` inventory. Those files remain in the
            // in-memory `compile_entry` used to seed Rolldown; the SDK target
            // source roots make them compiler inputs independently of the
            // loader.json handoff. Component API 23+ still appends the
            // generated ResourceTable at the task boundary.
            hvigor_compile_entry.retain(|entry| {
                Path::new(entry).file_name() == Some(OsStr::new("ResourceTable.ts"))
            });
        }
        for path in &serialized_compile_entry {
            if let Ok(rel) = path.strip_prefix(&src_main_root) {
                compile_entry.push(Self::compile_entry_from_src_main_rel(rel));
            } else {
                compile_entry.push(to_absolute_path_string(path));
            }
        }
        if matches!(config.output_target, OutputTarget::Test) {
            // The `src/test` suite participates in the local-test compiler
            // input surface (testability templates were added up front).
            let test_sources = module_layout.test_sources_dir();
            if test_sources.is_dir() {
                for file in Self::collect_code_files(&test_sources) {
                    compile_entry.push(to_absolute_path_string(&file));
                }
            }
        }

        let bytecode_har_deps = if collects_bytecode_har_inputs {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "bytecode_har_session_graph");
            ByteCodeHarMetadataWriter::scan_from_dependency_graph(dep_graph)
        } else {
            Vec::new()
        };
        let other_compile_paths = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "bytecode_other_compile_files");
            LoaderByteCodeHarFields::collect_other_compile_files_from_graph(
                &bytecode_har_deps,
                dep_graph,
            )
        };
        let other_compile_files: Vec<String> = other_compile_paths
            .iter()
            .map(|path| to_absolute_path_string(path))
            .collect();
        compile_entry.extend(other_compile_files.iter().cloned());

        // Deduplicate while preserving order.
        {
            let mut seen = FxHashSet::default();
            compile_entry.retain(|e| seen.insert(e.clone()));
        }
        if matches!(config.output_target, OutputTarget::Har) {
            for entry in &compile_entry {
                let path = Path::new(entry);
                let absolute = if path.has_root() {
                    to_absolute_path_string(path)
                } else {
                    to_absolute_path_string(&src_main_root.join(entry.trim_start_matches("./")))
                };
                // `GenerateLoaderJson.processByteCodeHar()` adds generated
                // ModuleInfo.ts to the serialized compiler entries. The
                // package `main` remains Rolldown's public input and is not
                // copied into loader.json merely because it is a root-level
                // file (the source fixture is `main: "Index.ets"`).
                if module_is_bytecode_har && package_main_entry.as_ref() == Some(&absolute) {
                    continue;
                }
                if !hvigor_compile_entry.contains(&absolute) {
                    hvigor_compile_entry.push(absolute);
                }
            }
        }

        let module_root = to_absolute_path_string(&module_cfg.src_path);
        let project_root = to_absolute_path_string(&config.project_root);
        let src_main = to_absolute_path_string(&module_layout.src_main());

        let build_mode_str = config.build_mode.as_str().to_string();

        let compile_mode = match manifest.compile_mode {
            arkdown_core::CompileMode::EsModule => "esmodule",
            arkdown_core::CompileMode::JsBundle => "jsbundle",
        }
        .to_string();

        let mut module_path_map = IndexMap::new();
        for project_module in &config.modules {
            module_path_map.insert(
                project_module.name.clone(),
                to_absolute_path_string(&project_module.src_path),
            );
        }
        if module_path_map.is_empty() {
            module_path_map.insert(module_cfg.name.clone(), module_root.clone());
        }

        let byte_code_har_info: IndexMap<String, ByteCodeHarEntry> = bytecode_har_deps
            .iter()
            .cloned()
            .map(|e| e.into_loader_entry())
            .collect();

        let declaration_entry = if use_normalized_ohm_url {
            let mut entries: Vec<String> = bytecode_har_deps
                .iter()
                .flat_map(|dep| dep.declaration_entry.iter().cloned())
                .collect();

            // D2: When this module is itself a ByteCode HAR (metadata.byteCodeHar=true),
            // generate its own declarationEntry from abilities + extension abilities.
            // This provides the OHM URLs that downstream consumers of THIS ByteCode HAR
            // will put into their own loader.json declarationEntry.
            let bundle_name = &config.product.bundle_name;
            let mod_name = &module_cfg.name;
            if module_is_bytecode_har {
                for ability_entry in &manifest.abilities {
                    let Some(path) = Self::source_entry_stem_path(&ability_entry.src_entry) else {
                        continue;
                    };
                    let ohm = Self::bundle_ohm_url(bundle_name, mod_name, &path);
                    entries.push(ohm);
                }
                for ext_ability in &manifest.extension_abilities {
                    let Some(path) = Self::source_entry_stem_path(&ext_ability.src_entry) else {
                        continue;
                    };
                    let ohm = Self::bundle_ohm_url(bundle_name, mod_name, &path);
                    entries.push(ohm);
                }
            }

            let mut seen = FxHashSet::default();
            entries.retain(|entry| seen.insert(entry.clone()));
            entries
        } else {
            Vec::new()
        };
        let update_version_info = if matches!(config.output_target, OutputTarget::Har) {
            IndexMap::new()
        } else {
            ByteCodeHarScanEntry::collect_bytecode_har_update_version_info_from_graph(
                &bytecode_har_deps,
                dep_graph,
            )
        };

        // --- TASK-LOADER-02: populate dynamicImportLibInfo from oh-package.json5 ---
        let dynamic_import_lib_info: IndexMap<String, serde_json::Value> = if module_is_bytecode_har
        {
            IndexMap::new()
        } else {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "dynamic_import_lib_info");
            let dynamic_dep_names = runtime_only_collector.collect_dynamic_import_dep_names();
            let entries = LoaderByteCodeHarFields::collect_dynamic_import_lib_info_from_graph(
                &module_cfg.name,
                &dynamic_dep_names,
                dep_graph,
            );
            entries
                .into_iter()
                .filter_map(|(k, v)| serde_json::to_value(v).ok().map(|json| (k, json)))
                .collect()
        };

        // --- D5: hspNameOhmMap/harNameOhmMap from direct oh_modules deps ---
        let (hsp_name_ohm_map, har_name_ohm_map) = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "hsp_har_ohm_maps");
            Self::collect_hsp_har_ohm_maps(config, module_cfg, dep_graph, module_is_bytecode_har)
        };

        let aot_fields = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "aot_loader_fields");
            AotLoaderFields::from_config(config, module_cfg, manifest, target_name)
        };
        let build_config_path = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "loader_build_config_path");
            Self::loader_build_config_path(config, module_cfg, target_name)
        };
        let build_config = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "loader_build_config_read");
            Self::read_loader_build_config(&build_config_path)
        };
        let patch_config = (!matches!(config.output_target, OutputTarget::Test))
            .then(|| {
                build_config
                    .as_ref()
                    .and_then(|config| config.get("patchConfig").cloned())
            })
            .flatten();
        let serialized_build_config_path = match config.output_target {
            // hvigor GenerateLoaderJson:
            // `(isPreview || isLocalTest()) && (c.buildConfigPath =
            // relative(modulePath, buildConfigPath))`.
            // The active ModulePathInfo build root is `build`, `.preview`, or
            // `.test`; `getBuildConfigPath()` always appends
            // `config/buildConfig.json` directly to that root.
            OutputTarget::Test => Some(".test/config/buildConfig.json".to_string()),
            OutputTarget::Preview => build_config_path
                .strip_prefix(&module_cfg.src_path)
                .map(ArtifactPathSerializer::rel_to_slash)
                .ok()
                .or_else(|| Some(to_absolute_path_string(&build_config_path))),
            _ => None,
        };

        let loader_out_root = module_layout.loader_out_default_dir();
        let node_modules_path = to_absolute_path_string(&loader_out_root.join("node_modules"));
        // Local unit-test builds run the `UnitTestArkTS` task; its cache
        // directory (`default@UnitTestArkTS`) participates in es2abc module
        // identities, so the task name must match hvigor exactly.
        let compile_task_name = if matches!(config.output_target, OutputTarget::Test) {
            "UnitTestArkTS"
        } else if matches!(config.output_target, OutputTarget::Preview) {
            "PreviewArkTS"
        } else if target_name == "ohosTest" {
            "OhosTestCompileArkTS"
        } else if matches!(config.output_target, OutputTarget::Har) {
            "HarCompileArkTS"
        } else {
            "CompileArkTS"
        };
        let compile_task_scope = if target_name == "ohosTest" {
            target_name
        } else {
            "default"
        };
        let compile_arkts_cache = module_layout
            .build_default()
            .join("cache")
            .join(compile_task_scope)
            .join(format!("{compile_task_scope}@{compile_task_name}"))
            .join(&compile_mode)
            .join(&build_mode_str);
        let cache_path = Self::absolute_dir_path_string(&compile_arkts_cache);
        // `UnitTestArkCompile.initDefaultArkCompileConfig()` replaces
        // `aceModuleBuild` with `pathInfo.getInterMediatesAssetsPath()/ets`.
        // This field is an Arkdown-only in-process handoff (it is skipped when
        // loader.json is serialized), so model the task-specific ProjectConfig
        // value directly instead of compiling into loader_out and copying it.
        let ace_module_build = if matches!(config.output_target, OutputTarget::Test) {
            Self::absolute_dir_path_string(&module_layout.test_assets_ets_dir())
        } else {
            Self::absolute_dir_path_string(&loader_out_root.join("ets"))
        };
        let hsp_resources_map = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "hsp_resources_map");
            Self::collect_hsp_resources_map(dep_graph)
        };
        let pkg_name_to_pkg_brief_info = {
            let _timer = LoaderTimingProbe::new(&module_cfg.name, "pkg_brief_info");
            Self::build_pkg_brief_info(config, module_cfg, dep_graph)
        };

        Ok(LoaderJson {
            module_path_map,
            compile_mode,
            project_root_path: project_root.clone(),
            node_modules_path,
            byte_code_har_info,
            declaration_entry,
            module_name: module_cfg.name.clone(),
            hsp_name_ohm_map,
            har_name_ohm_map,
            package_manager_type: "ohpm".to_string(),
            hvigor_compile_entry,
            other_compile_files,
            dynamic_import_lib_info,
            router_map,
            hsp_resources_map,
            update_version_info,
            customized_har: config
                .product
                .build_option
                .customized_har_base_package
                .is_some(),
            byte_code_har: module_is_bytecode_har.then_some(true),
            // The common loader.json never carries the widget-only field.
            pkg_name2_other_compile_files: None,
            an_build_out_put: aot_fields.an_build_out_put,
            an_build_mode: aot_fields.an_build_mode,
            ap_path: aot_fields.ap_path,
            build_config_path: serialized_build_config_path,
            patch_config,
            non_entry_importees: vec![],
            compiled_importees: vec![],

            compile_entry,
            form_entry_sources,
            insight_intent_form_bindings,
            runtime_entry_sources,
            page_entry_sources,
            module_root_path: src_main.clone(),
            cache_path,
            ace_module_build,
            build_mode: build_mode_str,
            // `OhosTestArkCompile.initDefaultArkCompileConfig()` forces
            // `supportChunks=false` independently of the inherited target
            // build option.
            support_chunks: target_name != "ohosTest"
                && config
                    .product
                    .build_option
                    .ark_options
                    .as_ref()
                    .is_some_and(|options| options.support_chunks),
            use_normalized_ohm_url,
            api_version: config.sdk_config.compile_api_version,
            fallback_an_build: aot_fields.fallback_an_build,
            pkg_name_to_pkg_brief_info,
            workers: discovered_workers,
            // GenerateLoaderJson does not serialize sourceRoots. HAR package
            // source roots belong to oh-package.json5 metadata and are added
            // later by ProcessHarArtifacts.processPackageJson.
            source_roots: Vec::new(),
            bundle_dependencies: {
                use arkdown_session::dependency_graph::snapshot::DependencyNodeType;
                dep_graph
                    .nodes
                    .values()
                    .filter(|node| {
                        matches!(
                            node.node_type,
                            DependencyNodeType::SourceHar
                                | DependencyNodeType::SharedHsp
                                | DependencyNodeType::IntegratedHsp
                        )
                    })
                    .map(|node| node.package_name.clone())
                    .collect()
            },
            global_dependencies: Vec::new(),
            compiler_types: Vec::new(),
            compiler_type_declaration_inputs: Vec::new(),
            dependency_pkg_version: Vec::new(),
            extra_importees: Vec::new(),
            compiled_package_names: Vec::new(),
            oh_exports: Vec::new(),
        })
    }
}
