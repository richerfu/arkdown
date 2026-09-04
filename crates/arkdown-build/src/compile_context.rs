//! Compile-environment, package-context and compile-context JSON builders.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use arkdown::BundleSourceMapPackageInfo;
use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_native::{
    NativeBuildSanitizers, NativeCmakeLocalDependency, NativeLibCollectionPolicy,
    NativeLibCollector, NativeLibPackageInfo,
};
use arkdown_pack::minify_package_intermediate_module_json;
use arkdown_path::{
    canonicalize_or_owned, lexical_normalize, ArtifactPathSerializer, NormalizedOhmUrl,
};
use arkdown_profile::{
    LoaderJson, PkgContextCurrentInput, PkgContextDependencyInput, PkgContextDependencyKind,
    PkgContextInfoInput, PkgContextInfoOpt, PkgContextModuleKind,
};
use arkdown_session::dependency_graph::snapshot::{DependencyNodeSnapshot, DependencyNodeType};
use arkdown_session::module::snapshot::OhosModuleType as SessionOhosModuleType;
use arkdown_session::{BuildSession, ModuleCompileContext, PackageSnapshot};
use rustc_hash::{FxHashMap, FxHashSet};

pub struct CompileEnvKeyBuilder<'a> {
    pub config: &'a ArkdownConfig,
    pub module: &'a ModuleConfig,
    pub loader_json: &'a LoaderJson,
    pub obfuscation_enabled: bool,
    pub target_name: &'a str,
    pub use_source_hash: bool,
    pub arkts_language_version: arkdown_core::ArktsLanguageVersion,
    pub preview_mode: bool,
    pub check_entry: bool,
    pub is_crossplatform: bool,
    pub ignore_crossplatform_check: bool,
    pub ui_transform_optimization: bool,
    pub use_get_current_bundle_name: bool,
}

impl<'a> CompileEnvKeyBuilder<'a> {
    pub fn new(
        config: &'a ArkdownConfig,
        module: &'a ModuleConfig,
        loader_json: &'a LoaderJson,
        obfuscation_enabled: bool,
        target_name: &'a str,
        use_source_hash: bool,
        arkts_language_version: arkdown_core::ArktsLanguageVersion,
    ) -> Self {
        Self {
            config,
            module,
            loader_json,
            obfuscation_enabled,
            target_name,
            use_source_hash,
            arkts_language_version,
            preview_mode: false,
            check_entry: false,
            is_crossplatform: false,
            ignore_crossplatform_check: false,
            ui_transform_optimization: false,
            use_get_current_bundle_name: false,
        }
    }

    pub fn with_preview_mode(mut self, preview_mode: bool) -> Self {
        self.preview_mode = preview_mode;
        self
    }

    pub fn with_check_entry(mut self, check_entry: bool) -> Self {
        self.check_entry = check_entry;
        self
    }

    pub fn with_crossplatform(mut self, enabled: bool, ignore_check: bool) -> Self {
        self.is_crossplatform = enabled;
        self.ignore_crossplatform_check = ignore_check;
        self
    }

    pub fn with_ui_transform_optimization(mut self, enabled: bool) -> Self {
        self.ui_transform_optimization = enabled;
        self
    }

    pub fn with_use_get_current_bundle_name(mut self, enabled: bool) -> Self {
        self.use_get_current_bundle_name = enabled;
        self
    }

    pub fn build(&self) -> Result<String> {
        let ets_loader_path =
            ArtifactPathSerializer::path_to_slash_string(&self.config.sdk_config.ets_loader_path)
                .map_err(|error| {
                anyhow::anyhow!("serialize ETS loader path for compile env failed: {error}")
            })?;
        let compiler_cache_path = arkdown_cache::compiler_cache_path(&self.loader_json.cache_path);

        Ok(format!(
      "{}#target={}#useSourceHash={}#arkts={}#preview={}#checkEntry={}#crossplatform={}#ignoreCrossplatformCheck={}#uiTransformOptimization={}#useGetCurrentBundleName={}",
      arkdown_cache::CompileEnv::from_plan(
        self.config.sdk_config.compile_sdk_version.clone(),
        self.config.product.compatible_sdk_version.clone(),
        self.config.product.runtime_os.clone(),
        ets_loader_path,
        self.obfuscation_enabled,
        format!("{:?}", self.module.module_type),
        compiler_cache_path,
      )
      .to_key_string(),
      self.target_name,
      self.use_source_hash,
      self.arkts_language_version,
      self.preview_mode,
      self.check_entry,
      self.is_crossplatform,
      self.ignore_crossplatform_check,
      self.ui_transform_optimization,
      self.use_get_current_bundle_name,
    ))
    }
}

pub struct PkgContextInfoInputBuilder<'a> {
    pub config: &'a ArkdownConfig,
    pub session: &'a BuildSession,
    pub module: &'a ModuleConfig,
    pub module_context: &'a ModuleCompileContext,
    pub excluded_package_roots: FxHashSet<PathBuf>,
    pub local_native_dependencies: Arc<[NativeCmakeLocalDependency]>,
}

impl<'a> PkgContextInfoInputBuilder<'a> {
    pub fn new(
        config: &'a ArkdownConfig,
        session: &'a BuildSession,
        module: &'a ModuleConfig,
        module_context: &'a ModuleCompileContext,
    ) -> Self {
        Self {
            config,
            session,
            module,
            module_context,
            excluded_package_roots: FxHashSet::default(),
            local_native_dependencies: Arc::from([]),
        }
    }

    pub fn excluding_package_roots(mut self, roots: &[PathBuf]) -> Self {
        self.excluded_package_roots = roots
            .iter()
            .map(|root| canonicalize_or_owned(root))
            .collect();
        self
    }

    pub fn with_local_native_dependencies(
        mut self,
        dependencies: Arc<[NativeCmakeLocalDependency]>,
    ) -> Self {
        self.local_native_dependencies = dependencies;
        self
    }

    pub fn is_excluded(&self, node: &DependencyNodeSnapshot) -> bool {
        node.package_root.as_ref().is_some_and(|root| {
            self.excluded_package_roots
                .contains(&canonicalize_or_owned(root))
        })
    }

    pub fn build(&self) -> Result<PkgContextInfoInput> {
        let package = self.module_context.module.package.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "GeneratePkgContextInfo requires oh-package.json5 snapshot for module '{}'",
                self.module.name
            )
        })?;
        let source_root = match self.module_context.module.module_type {
            SessionOhosModuleType::Entry | SessionOhosModuleType::Feature => {
                self.module_context.module.src_main.clone()
            }
            SessionOhosModuleType::Shared | SessionOhosModuleType::Har => self
                .module_context
                .module
                .source_roots
                .first()
                .cloned()
                .unwrap_or_else(|| self.module_context.module.src_main.clone()),
            SessionOhosModuleType::Unknown(ref module_type) => {
                anyhow::bail!(
                    "GeneratePkgContextInfo does not support unknown module type '{}'",
                    module_type
                )
            }
        };

        let mut dependencies = Vec::with_capacity(self.module_context.deps.nodes.len());
        for node in self.module_context.deps.nodes.values() {
            if self.is_excluded(node) {
                continue;
            }
            dependencies.push(PkgContextDependencyInput {
                alias: node.alias.clone(),
                package_name: node.package_name.clone(),
                version_spec: self.dependency_version(node)?,
                package_root: node.package_root.clone(),
                node_entry: node.package.as_deref().map(Self::dependency_node_entry),
                main: node.package.as_ref().and_then(|package| {
                    package
                        .package_json
                        .main
                        .clone()
                        .or_else(|| package.package_json.types.clone())
                }),
                kind: self.dependency_kind(node.node_type),
                module_name: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_name.clone()),
                bundle_name: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.bundle_name.clone()),
                bundle_type: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.bundle_type.clone()),
                oh_export_files: node
                    .package
                    .as_ref()
                    .and_then(|package| package.oh_export_files.clone()),
            });
        }
        let native_build_option = self.effective_native_build_option()?;
        let native_libraries = NativeLibCollector::new(
            self.module,
            &native_build_option.abi_filters,
            self.module_context.deps.as_ref(),
        )
        .excluding_dependency_roots(
            &self
                .excluded_package_roots
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
        )
        .with_policy(
            NativeLibCollectionPolicy::default()
                .with_native_lib_option(native_build_option.native_lib)
                .with_self_package(self.self_package_info())
                .with_cmake_local_dependency_inventory(Arc::clone(&self.local_native_dependencies)),
        )
        .collect_context_names();

        Ok(PkgContextInfoInput {
            current: PkgContextCurrentInput {
                package_name: package.name.clone(),
                bundle_name: self.session.project.bundle_name.clone(),
                bundle_type: self.session.project.bundle_type.clone(),
                module_kind: self.current_module_kind()?,
                module_root: self.module_context.module.root.clone(),
                source_root,
                main: package.package_json.main.clone(),
                version: package.version.clone(),
                oh_export_files: package.oh_export_files.clone(),
            },
            dependencies,
            native_libraries,
        })
    }

    /// Match hvigor-arkts-compose `findNodeEntryFiles()` entry-key priority.
    pub fn dependency_node_entry(package: &PackageSnapshot) -> String {
        for key in ["exports", "module", "jsnext:main"] {
            if let Some(entry) = package
                .package_json
                .extra
                .get(key)
                .and_then(Self::node_entry_from_value)
            {
                return entry;
            }
        }
        if let Some(main) = package
            .package_json
            .main
            .as_deref()
            .filter(|entry| !entry.is_empty())
        {
            return main.to_string();
        }
        "index.js".to_string()
    }

    /// Match `module-meta-info-plugin.ts#parseEntryFiles` and return the first
    /// candidate consumed by `dependency-manager.ts#getEntryMainFilePath`.
    pub fn node_entry_from_value(value: &serde_json::Value) -> Option<String> {
        match value {
            serde_json::Value::String(entry) => (!entry.is_empty()).then(|| entry.clone()),
            serde_json::Value::Array(entries) => {
                entries.iter().find_map(Self::node_entry_from_value)
            }
            serde_json::Value::Object(entries) => ["import", ".", "default", "require", "node"]
                .into_iter()
                .find_map(|key| entries.get(key).and_then(Self::node_entry_from_value)),
            _ => None,
        }
    }

    pub fn source_map_package_info(&self) -> Result<BundleSourceMapPackageInfo> {
        let package = self.module_context.module.package.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "SourceMapGenerator requires oh-package.json5 snapshot for module '{}'",
                self.module.name
            )
        })?;
        if package.name.is_empty() || package.version.is_empty() {
            anyhow::bail!(
                "SourceMapGenerator requires package name and version for module '{}'",
                self.module.name
            );
        }

        let mut dependency_package_versions = FxHashMap::default();
        for (name, version) in package.package_json.dev_dependencies.iter() {
            if !version.is_empty() {
                dependency_package_versions
                    .entry(name.clone())
                    .or_insert_with(|| version.clone());
            }
        }
        if let Some(root_package) = self.session.project.root_package.as_ref() {
            for (name, version) in root_package.package_json.dev_dependencies.iter() {
                if !version.is_empty() {
                    dependency_package_versions
                        .entry(name.clone())
                        .or_insert_with(|| version.clone());
                }
            }
        }
        for node in self.module_context.deps.nodes.values() {
            if self.is_excluded(node) {
                continue;
            }
            let Some(version) = node
                .package
                .as_ref()
                .map(|package| package.version.as_str())
                .filter(|version| !version.is_empty())
            else {
                continue;
            };
            dependency_package_versions
                .entry(node.package_name.clone())
                .or_insert_with(|| version.to_string());
            if node.alias != node.package_name {
                dependency_package_versions
                    .entry(node.alias.clone())
                    .or_insert_with(|| version.to_string());
            }
        }

        Ok(BundleSourceMapPackageInfo {
            entry_package_name: package.name.clone(),
            entry_package_version: package.version.clone(),
            dependency_package_versions,
        })
    }

    pub fn effective_native_build_option(
        &self,
    ) -> Result<arkdown_config::build_profile::EffectiveNativeBuildOption> {
        Ok(
            arkdown_config::build_profile::EffectiveNativeBuildOption::from_build_option(
                &self.config.product.build_option,
            ),
        )
    }

    pub fn self_package_info(&self) -> NativeLibPackageInfo {
        self.module_context
            .module
            .package
            .as_ref()
            .map(|package| {
                NativeLibPackageInfo::local(package.name.clone(), package.version.clone())
            })
            .unwrap_or_else(|| NativeLibPackageInfo::local(self.module.name.clone(), String::new()))
    }

    pub fn current_module_kind(&self) -> Result<PkgContextModuleKind> {
        Ok(match self.module_context.module.module_type {
            SessionOhosModuleType::Entry | SessionOhosModuleType::Feature => {
                PkgContextModuleKind::Hap
            }
            SessionOhosModuleType::Shared => PkgContextModuleKind::Hsp,
            SessionOhosModuleType::Har => PkgContextModuleKind::Har,
            SessionOhosModuleType::Unknown(ref module_type) => {
                anyhow::bail!(
                    "GeneratePkgContextInfo does not support unknown module type '{}'",
                    module_type
                )
            }
        })
    }

    pub fn dependency_kind(&self, node_type: DependencyNodeType) -> PkgContextDependencyKind {
        match node_type {
            DependencyNodeType::SharedHsp | DependencyNodeType::IntegratedHsp => {
                PkgContextDependencyKind::Hsp
            }
            DependencyNodeType::NativeSo => PkgContextDependencyKind::So,
            DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar => {
                PkgContextDependencyKind::Har
            }
            DependencyNodeType::OhpmSource | DependencyNodeType::Unknown => {
                PkgContextDependencyKind::Other
            }
        }
    }

    pub fn dependency_version(&self, node: &DependencyNodeSnapshot) -> Result<Option<String>> {
        match self.dependency_kind(node.node_type) {
            PkgContextDependencyKind::Hsp | PkgContextDependencyKind::So => Ok(None),
            PkgContextDependencyKind::Har | PkgContextDependencyKind::Other => {
                let package = node.package.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "GeneratePkgContextInfo requires package metadata for dependency '{}'",
                        node.package_name
                    )
                })?;
                if package.version.is_empty() {
                    anyhow::bail!(
                        "GeneratePkgContextInfo requires package version for dependency '{}'",
                        node.package_name
                    );
                }
                Ok(Some(package.version.clone()))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SanitizerProfileFlags {
    pub asan: bool,
    pub tsan: bool,
    pub hwasan: bool,
    pub ubsan: bool,
}

impl SanitizerProfileFlags {
    pub fn new(asan: bool, tsan: bool, hwasan: bool, ubsan: bool) -> Self {
        Self {
            asan,
            tsan,
            hwasan,
            ubsan,
        }
    }

    /// `AbstractMergeProfile` ORs exact extra-config predicates with the
    /// flattened effective CMake arguments for each target.
    pub fn with_cmake_arguments(
        self,
        external_native_options: Option<&arkdown_core::ExternalNativeOptions>,
    ) -> Self {
        let contains = |expected: &str| {
            external_native_options
                .into_iter()
                .flat_map(|options| options.arguments.iter())
                .flat_map(|argument| argument.split_whitespace())
                .any(|argument| argument == expected)
        };
        Self {
            asan: self.asan || contains("-DOHOS_ENABLE_ASAN=ON"),
            tsan: self.tsan || contains("-DOHOS_ENABLE_TSAN=ON"),
            hwasan: self.hwasan || contains("-DOHOS_ENABLE_HWASAN=ON"),
            ubsan: self.ubsan || contains("-DOHOS_ENABLE_UBSAN=ON"),
        }
    }

    pub fn native_sanitizers(self) -> NativeBuildSanitizers {
        NativeBuildSanitizers::new(self.asan, self.tsan, self.hwasan, self.ubsan)
    }

    pub fn enabled_count(self) -> usize {
        [self.asan, self.tsan, self.hwasan, self.ubsan]
            .into_iter()
            .filter(|enabled| *enabled)
            .count()
    }

    pub fn apply_to_module_json(&self, path: &Path) -> Result<()> {
        if self.enabled_count() == 0 {
            return Ok(());
        }
        let content = fs_err::read_to_string(path)?;
        let mut value: serde_json::Value = serde_json::from_str(&content)?;
        self.apply_to_value(&mut value)?;
        fs_err::write(path, serde_json::to_vec(&value)?)?;
        Ok(())
    }

    pub fn apply_to_value(&self, value: &mut serde_json::Value) -> Result<()> {
        if self.enabled_count() == 0 {
            return Ok(());
        }
        let root = value
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("module.json root must be an object"))?;
        let app = root
            .entry("app")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("module.json app must be an object"))?;
        if self.asan {
            app.insert("asanEnabled".to_string(), serde_json::Value::Bool(true));
        }
        if self.tsan {
            app.insert("tsanEnabled".to_string(), serde_json::Value::Bool(true));
        }
        if self.hwasan {
            app.insert("hwasanEnabled".to_string(), serde_json::Value::Bool(true));
        }
        if self.ubsan {
            app.insert("ubsanEnabled".to_string(), serde_json::Value::Bool(true));
        }
        Ok(())
    }
}

pub struct PackageModuleJsonOptions<'a> {
    pub path: &'a Path,
}

impl<'a> PackageModuleJsonOptions<'a> {
    pub fn new(path: &'a Path) -> Self {
        Self { path }
    }

    pub fn compress_native_libs(&self) -> Result<bool> {
        let content = fs_err::read_to_string(self.path)?;
        let value: serde_json::Value = serde_json::from_str(&content)?;
        Ok(value
            .get("module")
            .and_then(|module| module.get("compressNativeLibs"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false))
    }

    pub fn write_for_pack(
        &self,
        output: &Path,
        deduplicate_har: Option<bool>,
        preload_system_so_effective: bool,
    ) -> Result<PathBuf> {
        let content = fs_err::read_to_string(self.path)?;
        let mut value: serde_json::Value = serde_json::from_str(&content)?;
        let module = value
            .get_mut("module")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| anyhow::anyhow!("package module.json must contain a module object"))?;
        if preload_system_so_effective {
            module.entry("appStartup").or_insert_with(|| {
                serde_json::Value::String("$profile:startup_config".to_string())
            });
        }
        if let Some(deduplicate_har) = deduplicate_har {
            module.insert(
                "deduplicateHar".to_string(),
                serde_json::Value::Bool(deduplicate_har),
            );
        }
        if let Some(parent) = output.parent() {
            fs_err::create_dir_all(parent)?;
        }
        // GeneratePkgModuleJson is the PackageHap input in hvigor. It writes the
        // schema-ordered compact form before app_packing_tool parses it into its
        // HashMap-backed archive entry. Passing the resource-stage insertion order
        // through here changes collision order in that final serializer.
        let generated = serde_json::to_string(&value)?;
        let generated = minify_package_intermediate_module_json(&generated)?;
        fs_err::write(output, generated.as_bytes())?;
        Ok(output.to_path_buf())
    }
}

pub struct CompileContextInfoWriter<'a> {
    pub loader_json: &'a LoaderJson,
    pub module_source_files: &'a [arkdown_core::HvigorModuleSourceFile],
    pub bundle_type: &'a str,
    pub bundle_name: &'a str,
    /// `isFormWidgetModule`: the widget pass joins form card sources into the
    /// main es2abc input only on the form widget module itself.
    pub form_widget_module: bool,
    pub compile_entry_override: Option<Vec<String>>,
}

impl<'a> CompileContextInfoWriter<'a> {
    pub fn new(
        loader_json: &'a LoaderJson,
        module_source_files: &'a [arkdown_core::HvigorModuleSourceFile],
        _pkg_context_info: &'a BTreeMap<String, PkgContextInfoOpt>,
        bundle_type: &'a str,
        bundle_name: &'a str,
        form_widget_module: bool,
    ) -> Self {
        Self {
            loader_json,
            module_source_files,
            bundle_type,
            bundle_name,
            form_widget_module,
            compile_entry_override: None,
        }
    }

    pub fn with_compile_entry_override(mut self, entries: Option<&[String]>) -> Self {
        self.compile_entry_override = entries.map(<[String]>::to_vec);
        self
    }

    pub fn write(&self, output_path: &Path) -> Result<()> {
        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let hsp_pkg_names = self.hsp_pkg_names();
        let compile_entries = self.compile_entries(&hsp_pkg_names);
        let update_version_info = self
            .loader_json
            .update_version_info
            .iter()
            .map(|(package_name, value)| (package_name.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut value = serde_json::json!({
            "hspPkgNames": hsp_pkg_names,
            "compileEntries": compile_entries,
            "updateVersionInfo": update_version_info,
        });
        if matches!(self.bundle_type, "shared" | "appPlugin" | "appService") {
            value["needModifyRecord"] = serde_json::Value::Bool(true);
            value["bundleName"] = serde_json::Value::String(self.bundle_name.to_string());
        }
        fs_err::write(output_path, serde_json::to_vec(&value)?)?;
        Ok(())
    }

    pub fn hsp_pkg_names(&self) -> Vec<String> {
        let mut names = self
            .loader_json
            .hsp_name_ohm_map
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn compile_entries(&self, hsp_pkg_names: &[String]) -> Vec<String> {
        let hsp_names = hsp_pkg_names
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        // Latest SDK `ModuleMode.generateCompileContextInfo()` iterates
        // `projectConfig.entryObj` into a JavaScript Set. Preserve that first-seen
        // order: sorting roots can change initialization order for cyclic ESM
        // graphs even though the same record names remain present.
        let mut entries = Vec::new();
        let mut seen_entries = FxHashSet::default();
        let source_records = self.source_record_index();
        if let Some(compile_entry_override) = self.compile_entry_override.as_ref() {
            // SDK ModuleHotreloadMode replaces `projectConfig.entryObj` with the
            // changed path list before regenerating compileContextInfo.json.
            for source_entry in compile_entry_override {
                if let Some(record_name) =
                    self.record_name_for_source_entry(source_entry, &source_records)
                {
                    if seen_entries.insert(record_name.clone()) {
                        entries.push(record_name);
                    }
                }
            }
        } else {
            for source_entry in self.loader_json.hvigor_compile_entry.iter() {
                if let Some(record_name) =
                    self.record_name_for_source_entry(source_entry, &source_records)
                {
                    if seen_entries.insert(record_name.clone()) {
                        entries.push(record_name);
                    }
                }
            }
            let other_compile_files = self
                .loader_json
                .other_compile_files
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            for source_entry in self.loader_json.compile_entry.iter() {
                if other_compile_files.contains(source_entry.as_str()) {
                    continue;
                }
                if let Some(record_name) =
                    self.record_name_for_source_entry(source_entry, &source_records)
                {
                    if seen_entries.insert(record_name.clone()) {
                        entries.push(record_name);
                    }
                }
            }
            for runtime_entry in &self.loader_json.runtime_entry_sources {
                if let Some(record_name) =
                    self.record_name_for_source_entry(runtime_entry, &source_records)
                {
                    if seen_entries.insert(record_name.clone()) {
                        entries.push(record_name);
                    }
                }
            }
            // hvigor's widget pass (`needSubmitArkTsWidget`) compiles the form card
            // sources into the main es2abc input (`filesInfo.txt`) in addition to the
            // dedicated `widgets.abc` — but only on the form widget module itself.
            // The form extension module (HAP) keeps them out (`getFormJsonArr`
            // returns [] for `isFormExtensionModule`).
            if self.form_widget_module {
                for form_entry in &self.loader_json.form_entry_sources {
                    if let Some(record_name) =
                        self.record_name_for_source_entry(form_entry, &source_records)
                    {
                        if seen_entries.insert(record_name.clone()) {
                            entries.push(record_name);
                        }
                    }
                }
            }
        }
        for ohm_url in &self.loader_json.declaration_entry {
            if let Some(record_name) = self.record_name_from_ohm_url(ohm_url, &hsp_names) {
                if seen_entries.insert(record_name.clone()) {
                    entries.push(record_name);
                }
            }
        }
        for router_entry in &self.loader_json.router_map {
            if let Some(ohm_url) = router_entry
                .get("ohmurl")
                .and_then(serde_json::Value::as_str)
            {
                if let Some(record_name) = self.record_name_from_ohm_url(ohm_url, &hsp_names) {
                    if seen_entries.insert(record_name.clone()) {
                        entries.push(record_name);
                    }
                }
            }
        }
        // Hvigor writes compile entries as dependency roots. The ArkTS compiler
        // resolves static and dynamic imports from those roots; expanding the full
        // import closure here turns dependencies into roots and can perturb module
        // initialization order for cyclic ESM graphs.
        entries
    }

    pub fn source_record_index(&self) -> FxHashMap<SourcePathKey, &str> {
        let mut records = FxHashMap::default();
        for source in self.module_source_files {
            if let Some(key) = SourcePathKey::from_module_source(source) {
                records.entry(key).or_insert(source.record_name.as_str());
            }
        }
        records
    }

    pub fn record_name_for_source_entry(
        &self,
        source_entry: &str,
        source_records: &FxHashMap<SourcePathKey, &str>,
    ) -> Option<String> {
        let expected = SourcePathKey::from_source_entry(
            source_entry,
            Path::new(&self.loader_json.module_root_path),
        );
        source_records
            .get(&expected)
            .map(|record_name| (*record_name).to_string())
    }

    pub fn record_name_from_ohm_url(
        &self,
        ohm_url: &str,
        hsp_pkg_names: &BTreeSet<&str>,
    ) -> Option<String> {
        let normalized = NormalizedOhmUrl::parse(ohm_url)?;
        let package_name = normalized.package_name()?;
        if hsp_pkg_names.contains(package_name) {
            return None;
        }
        normalized.record_name()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourcePathKey {
    pub value: PathBuf,
}

impl SourcePathKey {
    pub fn from_source_entry(source_entry: &str, module_root: &Path) -> Self {
        let path = Path::new(source_entry);
        let value = if path.has_root() {
            path.to_path_buf()
        } else {
            module_root.join(Self::trim_relative_prefix(path))
        };
        Self::new(value)
    }

    pub fn from_module_source(source: &arkdown_core::HvigorModuleSourceFile) -> Option<Self> {
        let module_id_path = Path::new(&source.module_id);
        if module_id_path.has_root() {
            return Some(Self::new(module_id_path));
        }
        source
            .source_file
            .rsplit_once('|')
            .map(|(_, source_file)| Self::new(Path::new(source_file)))
    }

    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = canonicalize_or_owned(path.as_ref());
        Self {
            value: lexical_normalize(&path).with_extension(""),
        }
    }

    pub fn trim_relative_prefix(path: &Path) -> PathBuf {
        path.components()
            .filter(|component| !matches!(component, std::path::Component::CurDir))
            .collect()
    }
}
