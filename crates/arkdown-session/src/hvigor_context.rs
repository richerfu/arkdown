//! Rust-owned projections for the public OHOS hvigor plugin contexts.
//!
//! Upstream correspondence:
//! - `TaskService.getOhpmDependencyInfo`
//! - `TaskService.getOhpmRemoteHspDependencies`
//! - `GlobalProjectDataService.initAllModuleNameHash`

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use arkdown_config::RuntimeContextOverrides;
use arkdown_path::canonicalize_or_owned;
use indexmap::IndexMap;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::dependency_graph::builder::{DependencyGraphBuilder, DependencySnapshotInventory};
use crate::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};

#[derive(Debug, Error)]
pub enum HvigorContextInspectError {
    #[error("required file not found: {0}")]
    FileNotFound(PathBuf),
    #[error("failed to read hvigor context input {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse hvigor context input {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("project path has no file name: {0}")]
    ProjectName(PathBuf),
    #[error("hvigor context snapshot does not contain module: {0}")]
    ModuleNotFound(PathBuf),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorContextSnapshot {
    pub all_module_name_hash: String,
    pub project: HvigorDependencyContextSnapshot,
    pub modules: BTreeMap<PathBuf, HvigorDependencyContextSnapshot>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorDependencyContextSnapshot {
    pub ohpm_dependency_info: IndexMap<String, OhpmDependencyInfo>,
    pub remote_hsp_dependency_info: IndexMap<String, OhpmDependencyInfo>,
    pub signed_remote_hsp_dependency_info: IndexMap<String, OhpmDependencyInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OhpmDependencyInfo {
    pub name: String,
    pub version: String,
    pub dependencies: IndexMap<String, String>,
    pub package_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_hsp_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_remote_hsp_path: Option<PathBuf>,
}

/// Stable diagnostic returned when an OHOS plugin context rejects an
/// in-memory model mutation. The codes and validation order follow the
/// corresponding latest-hvigor loaders rather than Node/V8 coercion rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct HvigorContextMutationDiagnostic {
    pub code: String,
    pub message: String,
}

/// A module package model captured when the JavaScript plugin facade is
/// created. The session owns subsequent package mutations and only returns
/// immutable snapshots to V8.
#[derive(Debug, Clone)]
pub struct HvigorContextModuleState {
    pub root: PathBuf,
    pub package: Value,
}

/// Result of an accepted context mutation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorContextMutation {
    pub value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_product_name: Option<String>,
}

/// Dynamic build-directory inputs read from Hvigor's parameter object.
///
/// These values are supplied for each path projection because custom plugins
/// can call `Parameter.setProperty()` while their hvigorfiles are evaluated.
#[derive(Debug, Clone, Default)]
pub struct HvigorContextPathOptions {
    pub build_cache_dir: Option<String>,
    pub build_dir: Option<String>,
    pub preview_mode: bool,
    pub local_test: bool,
    pub ohos_test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorProjectContextPaths {
    pub build_root_path: PathBuf,
    pub build_product_output_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorModuleTargetContextPaths {
    pub build_product_root_path: PathBuf,
    pub module_build_path: PathBuf,
    pub build_target_output_path: PathBuf,
    pub intermediates_res_path: PathBuf,
    pub module_build_cache_path: PathBuf,
}

/// Rust owner for mutable state exposed through `OhosProjectContextImpl` and
/// `OhosModuleContextImpl`.
///
/// JavaScript still supplies the method-shaped public facade required by
/// arbitrary Hvigor plugins. Model validation, package mutation, canonical
/// module lookup, dependency snapshot caching, and path projection remain in
/// this session.
pub struct HvigorContextSession {
    project_root: PathBuf,
    product_name: String,
    product_is_explicit: bool,
    project_profile: Value,
    project_package: Value,
    module_roots: BTreeMap<PathBuf, PathBuf>,
    module_packages: BTreeMap<PathBuf, Value>,
    runtime_overrides: RuntimeContextOverrides,
    snapshot: Option<HvigorContextSnapshot>,
}

impl HvigorContextSession {
    pub fn new(
        project_root: impl Into<PathBuf>,
        product_name: impl Into<String>,
        product_is_explicit: bool,
        project_profile: Value,
        project_package: Value,
        modules: Vec<HvigorContextModuleState>,
    ) -> Self {
        // Public Hvigor path getters preserve the spelling supplied by the
        // project model (for example `/var` on macOS). Canonicalization is
        // restricted to map identity and dependency inspection below.
        let project_root = arkdown_path::lexical_normalize(&project_root.into());
        let mut module_roots = BTreeMap::new();
        let mut module_packages = BTreeMap::new();
        for module in modules {
            let public_root = arkdown_path::lexical_normalize(&module.root);
            let identity = canonicalize_or_owned(&public_root);
            module_roots.insert(identity.clone(), public_root);
            module_packages.insert(identity, module.package);
        }
        Self {
            project_root,
            product_name: product_name.into(),
            product_is_explicit,
            project_profile,
            project_package,
            module_roots,
            module_packages,
            runtime_overrides: RuntimeContextOverrides::default(),
            snapshot: None,
        }
    }

    pub fn runtime_overrides(&self) -> Option<RuntimeContextOverrides> {
        (!self.runtime_overrides.is_empty()).then(|| self.runtime_overrides.clone())
    }

    pub fn project_package(&self) -> &Value {
        &self.project_package
    }

    pub fn module_package_snapshot(&self, root: &Path) -> Option<Value> {
        self.module_package(root).cloned()
    }

    pub fn register_module(&mut self, root: impl Into<PathBuf>, package: Value) {
        let public_root = arkdown_path::lexical_normalize(&root.into());
        let identity = canonicalize_or_owned(&public_root);
        self.module_roots.insert(identity.clone(), public_root);
        self.module_packages.insert(identity, package);
        self.invalidate_snapshot();
    }

    pub fn set_project_build_profile(
        &mut self,
        profile: Value,
    ) -> Result<HvigorContextMutation, HvigorContextMutationDiagnostic> {
        self.validate_project_products(&profile)?;
        self.project_profile = profile.clone();
        self.runtime_overrides.project.build_profile = Some(profile.clone());
        if !self.product_is_explicit {
            self.product_name = Self::products(&profile)
                .and_then(|products| products.first())
                .and_then(|product| product.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("default")
                .to_string();
        }
        self.invalidate_snapshot();
        Ok(HvigorContextMutation {
            value: profile,
            selected_product_name: Some(self.product_name.clone()),
        })
    }

    pub fn set_project_app_json(&mut self, app_json: Value) -> HvigorContextMutation {
        self.runtime_overrides.project.app_json = Some(app_json.clone());
        self.invalidate_snapshot();
        HvigorContextMutation {
            value: app_json,
            selected_product_name: None,
        }
    }

    pub fn set_module_build_profile(
        &mut self,
        root: &Path,
        profile: Value,
    ) -> HvigorContextMutation {
        self.module_overrides_mut(root).build_profile = Some(profile.clone());
        self.invalidate_snapshot();
        HvigorContextMutation {
            value: profile,
            selected_product_name: None,
        }
    }

    pub fn set_module_json(&mut self, root: &Path, module_json: Value) -> HvigorContextMutation {
        self.module_overrides_mut(root).module_json = Some(module_json.clone());
        self.invalidate_snapshot();
        HvigorContextMutation {
            value: module_json,
            selected_product_name: None,
        }
    }

    pub fn project_package_field(&self, field: &str) -> Option<Value> {
        self.project_package.get(field).cloned()
    }

    pub fn module_package_field(&self, root: &Path, field: &str) -> Option<Value> {
        self.module_package(root)?.get(field).cloned()
    }

    pub fn set_project_package_field(
        &mut self,
        field: &str,
        value: Value,
    ) -> Result<HvigorContextMutation, HvigorContextMutationDiagnostic> {
        Self::validate_package_field(field, &value)?;
        Self::replace_object_field(&mut self.project_package, field, value)?;
        let package = self.project_package.clone();
        self.runtime_overrides.project.package = Some(package.clone());
        self.invalidate_snapshot();
        Ok(HvigorContextMutation {
            value: package,
            selected_product_name: None,
        })
    }

    pub fn set_module_package_field(
        &mut self,
        root: &Path,
        field: &str,
        value: Value,
    ) -> Result<HvigorContextMutation, HvigorContextMutationDiagnostic> {
        Self::validate_package_field(field, &value)?;
        let root = canonicalize_or_owned(root);
        let package = self
            .module_packages
            .entry(root.clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        Self::replace_object_field(package, field, value)?;
        let package = package.clone();
        self.module_overrides_mut(&root).package = Some(package.clone());
        self.invalidate_snapshot();
        Ok(HvigorContextMutation {
            value: package,
            selected_product_name: None,
        })
    }

    pub fn set_module_version(
        &mut self,
        root: &Path,
        version: Value,
    ) -> Result<HvigorContextMutation, HvigorContextMutationDiagnostic> {
        if !version.is_string() {
            return Err(Self::diagnostic(
                "VERSION_IS_NOT_A_STRING",
                format!("version is not a string: {}", Self::display_value(&version)),
            ));
        }
        self.set_module_package_field(root, "version", version)
    }

    pub fn dependency_context(
        &mut self,
        root: Option<&Path>,
    ) -> Result<HvigorDependencyContextSnapshot, HvigorContextInspectError> {
        let snapshot = self.snapshot()?;
        match root {
            None => Ok(snapshot.project.clone()),
            Some(root) => {
                let normalized = canonicalize_or_owned(root);
                snapshot
                    .modules
                    .iter()
                    .find_map(|(candidate, context)| {
                        (canonicalize_or_owned(candidate) == normalized).then(|| context.clone())
                    })
                    .ok_or_else(|| HvigorContextInspectError::ModuleNotFound(root.to_path_buf()))
            }
        }
    }

    pub fn all_module_name_hash(&mut self) -> Result<String, HvigorContextInspectError> {
        Ok(self.snapshot()?.all_module_name_hash.clone())
    }

    pub fn project_paths(
        &self,
        options: &HvigorContextPathOptions,
    ) -> Result<HvigorProjectContextPaths, HvigorContextInspectError> {
        HvigorContextPathPlanner::new(&self.project_root, options).project_paths(&self.product_name)
    }

    pub fn module_target_paths(
        &self,
        module_root: &Path,
        target_name: &str,
        options: &HvigorContextPathOptions,
    ) -> Result<HvigorModuleTargetContextPaths, HvigorContextInspectError> {
        HvigorContextPathPlanner::new(&self.project_root, options).module_target_paths(
            module_root,
            &self.product_name,
            target_name,
        )
    }

    fn snapshot(&mut self) -> Result<&HvigorContextSnapshot, HvigorContextInspectError> {
        if self.snapshot.is_none() {
            self.snapshot = Some(
                HvigorContextInspector::new(
                    &self.project_root,
                    &self.product_name,
                    Arc::new(self.runtime_overrides.clone()),
                )
                .inspect()?,
            );
        }
        Ok(self.snapshot.as_ref().expect("snapshot initialized above"))
    }

    fn validate_project_products(
        &self,
        next: &Value,
    ) -> Result<(), HvigorContextMutationDiagnostic> {
        let Some(current_products) = Self::products(&self.project_profile) else {
            return Err(Self::diagnostic(
                "CAN_NOT_ADD_OR_DELETE_PRODUCTS",
                "Products cannot be added to or removed from build-profile.json5 through OhosProjectContext.",
            ));
        };
        let Some(next_products) = Self::products(next) else {
            return Err(Self::diagnostic(
                "CAN_NOT_ADD_OR_DELETE_PRODUCTS",
                "Products cannot be added to or removed from build-profile.json5 through OhosProjectContext.",
            ));
        };
        if current_products.len() != next_products.len() {
            return Err(Self::diagnostic(
                "CAN_NOT_ADD_OR_DELETE_PRODUCTS",
                "Products cannot be added to or removed from build-profile.json5 through OhosProjectContext.",
            ));
        }
        for current in current_products {
            let name = current.get("name").and_then(Value::as_str);
            let candidate = next_products
                .iter()
                .find(|product| product.get("name").and_then(Value::as_str) == name);
            if candidate.is_none_or(|product| product.get("runtimeOS") != current.get("runtimeOS"))
            {
                return Err(Self::diagnostic(
                    "CAN_NOT_CHANGE_NAME_OR_RUNTIME_PRODUCTS",
                    format!(
                        "The product name or runtimeOS cannot be changed: {}.",
                        serde_json::to_string(&name).unwrap_or_else(|_| "null".to_string())
                    ),
                ));
            }
        }
        Ok(())
    }

    fn products(profile: &Value) -> Option<&Vec<Value>> {
        profile.pointer("/app/products").and_then(Value::as_array)
    }

    fn validate_package_field(
        field: &str,
        value: &Value,
    ) -> Result<(), HvigorContextMutationDiagnostic> {
        if matches!(
            field,
            "dependencies" | "devDependencies" | "dynamicDependencies" | "overrides"
        ) {
            let values: Box<dyn Iterator<Item = &Value> + '_> = match value {
                Value::Object(values) => Box::new(values.values()),
                Value::Array(values) => Box::new(values.iter()),
                _ => Box::new(std::iter::empty()),
            };
            if let Some(version) = values.into_iter().find(|version| !version.is_string()) {
                return Err(Self::diagnostic(
                    "DEPENDENCY_VERSION_IS_NOT_A_STRING",
                    format!(
                        "dependency version is not a string: {}",
                        Self::display_value(version)
                    ),
                ));
            }
        }
        Ok(())
    }

    fn replace_object_field(
        object: &mut Value,
        field: &str,
        value: Value,
    ) -> Result<(), HvigorContextMutationDiagnostic> {
        let Some(object) = object.as_object_mut() else {
            return Err(Self::diagnostic(
                "INVALID_OH_PACKAGE_MODEL",
                "oh-package.json5 runtime model must be an object",
            ));
        };
        object.insert(field.to_string(), value);
        Ok(())
    }

    fn module_package(&self, root: &Path) -> Option<&Value> {
        let normalized = canonicalize_or_owned(root);
        self.module_packages.get(&normalized)
    }

    fn module_overrides_mut(
        &mut self,
        root: &Path,
    ) -> &mut arkdown_config::RuntimeModuleContextOverrides {
        let identity = canonicalize_or_owned(root);
        let public_root = self
            .module_roots
            .get(&identity)
            .cloned()
            .unwrap_or_else(|| arkdown_path::lexical_normalize(root));
        self.runtime_overrides
            .modules
            .entry(public_root)
            .or_default()
    }

    fn invalidate_snapshot(&mut self) {
        self.snapshot = None;
    }

    fn diagnostic(code: &str, message: impl Into<String>) -> HvigorContextMutationDiagnostic {
        HvigorContextMutationDiagnostic {
            code: code.to_string(),
            message: message.into(),
        }
    }

    fn display_value(value: &Value) -> String {
        match value {
            Value::String(value) => value.clone(),
            _ => value.to_string(),
        }
    }
}

struct HvigorContextPathPlanner<'a> {
    project_root: &'a Path,
    options: &'a HvigorContextPathOptions,
}

impl<'a> HvigorContextPathPlanner<'a> {
    fn new(project_root: &'a Path, options: &'a HvigorContextPathOptions) -> Self {
        Self {
            project_root,
            options,
        }
    }

    fn project_paths(
        &self,
        product_name: &str,
    ) -> Result<HvigorProjectContextPaths, HvigorContextInspectError> {
        let project_name = self.project_name()?;
        let parent = self.build_cache_parent(self.project_root, Path::new(&project_name), false);
        let build_root_path = arkdown_path::lexical_normalize(&parent.join("build"));
        let build_product_output_path = build_root_path.join("outputs").join(product_name);
        Ok(HvigorProjectContextPaths {
            build_root_path,
            build_product_output_path,
        })
    }

    fn module_target_paths(
        &self,
        module_root: &Path,
        product_name: &str,
        target_name: &str,
    ) -> Result<HvigorModuleTargetContextPaths, HvigorContextInspectError> {
        let module_root = arkdown_path::lexical_normalize(module_root);
        let relative_root =
            PathBuf::from(self.project_name()?).join(self.relative_module_path(&module_root));
        let cache_parent = self.build_cache_parent(&module_root, &relative_root, true);
        let output_parent = self.build_cache_parent(&module_root, &relative_root, false);
        let module_build_path = arkdown_path::lexical_normalize(&cache_parent.join("build"));
        let module_product_path = module_build_path.join(product_name);
        let build_target_output_path = arkdown_path::lexical_normalize(
            &output_parent
                .join("build")
                .join(product_name)
                .join("outputs")
                .join(target_name),
        );
        Ok(HvigorModuleTargetContextPaths {
            // `path.resolve(cacheParent, absoluteModuleRoot)` in
            // OhosModuleContextImpl always selects the absolute module root.
            build_product_root_path: module_root,
            module_build_path,
            build_target_output_path,
            intermediates_res_path: module_product_path
                .join("intermediates")
                .join("res")
                .join(target_name),
            module_build_cache_path: module_product_path.join("cache").join(target_name),
        })
    }

    fn build_cache_parent(
        &self,
        local_root: &Path,
        relative_root: &Path,
        support_build_cache_dir: bool,
    ) -> PathBuf {
        let Some(configured) = self.configured_build_directory(support_build_cache_dir) else {
            return local_root.to_path_buf();
        };
        if !configured.is_absolute() {
            return local_root.to_path_buf();
        }
        let configured = if configured.exists() {
            canonicalize_or_owned(&configured)
        } else {
            arkdown_path::lexical_normalize(&configured)
        };
        arkdown_path::lexical_normalize(&configured.join(relative_root))
    }

    fn configured_build_directory(&self, support_build_cache_dir: bool) -> Option<PathBuf> {
        let mut configured = self.options.build_cache_dir.as_deref();
        if configured.is_none() || support_build_cache_dir {
            configured = configured.or(self.options.build_dir.as_deref());
            if configured.is_some()
                && !self.options.preview_mode
                && !self.options.local_test
                && !self.options.ohos_test
            {
                return configured.map(Self::unquote_path).map(PathBuf::from);
            }
        }
        None
    }

    fn project_name(&self) -> Result<String, HvigorContextInspectError> {
        self.project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| HvigorContextInspectError::ProjectName(self.project_root.to_path_buf()))
    }

    fn relative_module_path(&self, module_root: &Path) -> PathBuf {
        if let Ok(relative) = module_root.strip_prefix(self.project_root) {
            return relative.to_path_buf();
        }
        let base = self.project_root.components().collect::<Vec<_>>();
        let target = module_root.components().collect::<Vec<_>>();
        let common = base
            .iter()
            .zip(&target)
            .take_while(|(left, right)| left == right)
            .count();
        if common == 0
            || matches!(base.first(), Some(Component::Prefix(_))) && base.first() != target.first()
        {
            let separator = std::path::MAIN_SEPARATOR;
            let sanitized = module_root
                .to_string_lossy()
                .replace(':', &separator.to_string());
            return PathBuf::from(sanitized.trim_start_matches(['/', '\\']));
        }
        let mut relative = PathBuf::new();
        for component in &base[common..] {
            if matches!(component, Component::Normal(_)) {
                relative.push("..");
            }
        }
        for component in &target[common..] {
            relative.push(component.as_os_str());
        }
        relative
    }

    fn unquote_path(value: &str) -> &str {
        value
            .strip_prefix('\'')
            .and_then(|value| value.strip_suffix('\''))
            .or_else(|| {
                value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
            })
            .unwrap_or(value)
    }
}

/// Inspects context data synchronously because hvigor's public getters are
/// synchronous. All filesystem and graph behavior remains in Rust.
pub struct HvigorContextInspector {
    project_root: PathBuf,
    product_name: String,
    runtime_overrides: Arc<RuntimeContextOverrides>,
}

impl HvigorContextInspector {
    pub fn new(
        project_root: impl Into<PathBuf>,
        product_name: impl Into<String>,
        runtime_overrides: Arc<RuntimeContextOverrides>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            product_name: product_name.into(),
            runtime_overrides,
        }
    }

    pub fn inspect(self) -> Result<HvigorContextSnapshot, HvigorContextInspectError> {
        let project_root = canonicalize_or_owned(&self.project_root);
        let profile = self.project_profile(&project_root)?;
        let modules = self.module_descriptors(&project_root, &profile);
        let remote_hsps = RemoteHspInventory::discover(&project_root, &modules)?;
        let mut dependency_inventory = DependencySnapshotInventory::default();

        let project_graph = DependencyGraphBuilder::new(
            self.project_name(&project_root)?,
            project_root.clone(),
            project_root.clone(),
        )
        .with_runtime_overrides(Arc::clone(&self.runtime_overrides))
        .build_with_inventory(&mut dependency_inventory);
        let project =
            self.context_for_graph(&project_graph, &project_root, &project_root, &remote_hsps);

        let mut module_contexts = BTreeMap::new();
        for module in &modules {
            let graph = DependencyGraphBuilder::new(
                module.name.clone(),
                project_root.clone(),
                module.root.clone(),
            )
            .with_runtime_overrides(Arc::clone(&self.runtime_overrides))
            .build_with_inventory(&mut dependency_inventory);
            module_contexts.insert(
                module.root.clone(),
                self.context_for_graph(&graph, &module.root, &project_root, &remote_hsps),
            );
        }

        Ok(HvigorContextSnapshot {
            all_module_name_hash: self.all_module_name_hash(&project_root, &modules)?,
            project,
            modules: module_contexts,
        })
    }

    fn project_profile(
        &self,
        project_root: &Path,
    ) -> Result<serde_json::Value, HvigorContextInspectError> {
        if let Some(value) = self.runtime_overrides.project.build_profile.as_ref() {
            return Ok(value.clone());
        }
        let path = project_root.join("build-profile.json5");
        if !path.is_file() {
            return Err(HvigorContextInspectError::FileNotFound(path));
        }
        let content =
            std::fs::read_to_string(&path).map_err(|source| HvigorContextInspectError::Io {
                path: path.clone(),
                source,
            })?;
        json5::from_str(&content).map_err(|error| HvigorContextInspectError::Parse {
            path,
            message: error.to_string(),
        })
    }

    fn module_descriptors(
        &self,
        project_root: &Path,
        profile: &serde_json::Value,
    ) -> Vec<HvigorModuleDescriptor> {
        profile
            .get("modules")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|module| {
                let name = module.get("name")?.as_str()?.to_string();
                let src_path = module
                    .get("srcPath")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| name.clone());
                Some(HvigorModuleDescriptor {
                    name,
                    root: canonicalize_or_owned(&project_root.join(&src_path)),
                })
            })
            .collect()
    }

    fn project_name(&self, project_root: &Path) -> Result<String, HvigorContextInspectError> {
        project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| HvigorContextInspectError::ProjectName(project_root.to_path_buf()))
    }

    fn context_for_graph(
        &self,
        graph: &DependencyGraphSnapshot,
        node_root: &Path,
        project_root: &Path,
        remote_hsps: &RemoteHspInventory,
    ) -> HvigorDependencyContextSnapshot {
        let mut context = HvigorDependencyContextSnapshot::default();
        for edge in &graph.edges {
            if context.ohpm_dependency_info.contains_key(&edge.alias) {
                continue;
            }
            let Some(node) = graph.nodes.get(&edge.package_name) else {
                continue;
            };
            let Some(package) = node.package.as_ref() else {
                continue;
            };
            context.ohpm_dependency_info.insert(
                edge.alias.clone(),
                OhpmDependencyInfo {
                    name: node.package_name.clone(),
                    version: package.version.clone(),
                    dependencies: package.package_json.dependencies.clone(),
                    package_path: edge.package_root.clone(),
                    remote_hsp_path: None,
                    signed_remote_hsp_path: None,
                },
            );
        }

        for edge in &graph.edges {
            if context.remote_hsp_dependency_info.contains_key(&edge.alias) {
                continue;
            }
            let Some(node) = graph.nodes.get(&edge.package_name) else {
                continue;
            };
            let Some(info) = self.remote_hsp_info(
                node,
                &edge.alias,
                node_root,
                project_root,
                remote_hsps,
                false,
            ) else {
                continue;
            };
            context
                .remote_hsp_dependency_info
                .insert(edge.alias.clone(), info);
            if let Some(info) = self.remote_hsp_info(
                node,
                &edge.alias,
                node_root,
                project_root,
                remote_hsps,
                true,
            ) {
                context
                    .signed_remote_hsp_dependency_info
                    .insert(edge.alias.clone(), info);
            }
        }
        context
    }

    fn remote_hsp_info(
        &self,
        node: &DependencyNodeSnapshot,
        _alias: &str,
        node_root: &Path,
        project_root: &Path,
        remote_hsps: &RemoteHspInventory,
        signed: bool,
    ) -> Option<OhpmDependencyInfo> {
        if node.node_type != DependencyNodeType::SharedHsp {
            return None;
        }
        let package_root = node.package_root.as_ref()?;
        let hsp_dir_name = RemoteHspInventory::dependency_directory_name(package_root)?;
        let hsp = remote_hsps.by_package_name.get(&node.package_name)?;
        let package = node.package.as_ref()?;
        let integrated = package
            .package_json
            .extra
            .get("metadata")
            .and_then(|metadata| metadata.get("integratedHsp"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let (remote_hsp_path, signed_remote_hsp_path) = if signed {
            (
                None,
                Some(
                    project_root
                        .join("build")
                        .join("cache")
                        .join(&self.product_name)
                        .join("remote_hsp")
                        .join(hsp_dir_name)
                        .join(hsp.file_name.replace(".hsp", "-signed.hsp")),
                ),
            )
        } else {
            let base = if integrated {
                project_root
                    .join("build")
                    .join("cache")
                    .join(&self.product_name)
                    .join("integrated_hsp")
            } else {
                node_root.join("oh_modules").join(".hsp")
            };
            (Some(base.join(hsp_dir_name).join(&hsp.file_name)), None)
        };
        Some(OhpmDependencyInfo {
            name: node.package_name.clone(),
            version: package.version.clone(),
            dependencies: package.package_json.dependencies.clone(),
            package_path: package_root.clone(),
            remote_hsp_path,
            signed_remote_hsp_path,
        })
    }

    fn all_module_name_hash(
        &self,
        project_root: &Path,
        modules: &[HvigorModuleDescriptor],
    ) -> Result<String, HvigorContextInspectError> {
        let mut names = String::new();
        for module in modules {
            names.push_str(&self.source_set_module_name(module, "main")?);
            let profile = self.module_build_profile(module)?;
            let has_ohos_test = profile
                .get("targets")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|targets| {
                    targets.iter().any(|target| {
                        target.get("name").and_then(serde_json::Value::as_str) == Some("ohosTest")
                    })
                });
            if has_ohos_test {
                names.push_str(&self.source_set_module_name(module, "ohosTest")?);
            }
        }
        names.push_str(&self.project_name(project_root)?);
        let digest = Sha256::digest(names.as_bytes());
        Ok(format!("{digest:x}"))
    }

    fn source_set_module_name(
        &self,
        module: &HvigorModuleDescriptor,
        source_set: &str,
    ) -> Result<String, HvigorContextInspectError> {
        let runtime_value = (source_set == "main")
            .then(|| self.runtime_overrides.module_for_root(&module.root))
            .flatten()
            .and_then(|runtime| runtime.module_json.as_ref());
        let path = module
            .root
            .join("src")
            .join(source_set)
            .join("module.json5");
        let value = if let Some(value) = runtime_value {
            Some(value.clone())
        } else if path.is_file() {
            let content =
                std::fs::read_to_string(&path).map_err(|source| HvigorContextInspectError::Io {
                    path: path.clone(),
                    source,
                })?;
            Some(
                json5::from_str(&content).map_err(|error| HvigorContextInspectError::Parse {
                    path: path.clone(),
                    message: error.to_string(),
                })?,
            )
        } else {
            None
        };
        Ok(value
            .as_ref()
            .and_then(|value: &serde_json::Value| value.pointer("/module/name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(module.name.as_str())
            .to_string())
    }

    fn module_build_profile(
        &self,
        module: &HvigorModuleDescriptor,
    ) -> Result<serde_json::Value, HvigorContextInspectError> {
        if let Some(value) = self
            .runtime_overrides
            .module_for_root(&module.root)
            .and_then(|runtime| runtime.build_profile.as_ref())
        {
            return Ok(value.clone());
        }
        let path = module.root.join("build-profile.json5");
        if !path.is_file() {
            return Ok(serde_json::Value::Null);
        }
        let content =
            std::fs::read_to_string(&path).map_err(|source| HvigorContextInspectError::Io {
                path: path.clone(),
                source,
            })?;
        json5::from_str(&content).map_err(|error| HvigorContextInspectError::Parse {
            path,
            message: error.to_string(),
        })
    }
}

#[derive(Debug)]
struct HvigorModuleDescriptor {
    name: String,
    root: PathBuf,
}

#[derive(Debug, Default)]
struct RemoteHspInventory {
    by_package_name: BTreeMap<String, RemoteHspArtifact>,
}

#[derive(Debug)]
struct RemoteHspArtifact {
    version: Version,
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct RemoteHspPackage {
    name: String,
    version: String,
}

impl RemoteHspInventory {
    fn discover(
        project_root: &Path,
        modules: &[HvigorModuleDescriptor],
    ) -> Result<Self, HvigorContextInspectError> {
        let mut inventory = Self::default();
        for root in modules
            .iter()
            .map(|module| module.root.as_path())
            .chain(std::iter::once(project_root))
        {
            inventory.scan_directory(&root.join("oh_modules").join(".hsp"))?;
        }
        Ok(inventory)
    }

    fn scan_directory(&mut self, root: &Path) -> Result<(), HvigorContextInspectError> {
        if !root.is_dir() {
            return Ok(());
        }
        let entries = std::fs::read_dir(root).map_err(|source| HvigorContextInspectError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| HvigorContextInspectError::Io {
                path: root.to_path_buf(),
                source,
            })?;
            let directory = entry.path();
            if !directory.is_dir() {
                continue;
            }
            let package_path = directory.join("oh-package.json5");
            if !package_path.is_file() {
                continue;
            }
            let content = std::fs::read_to_string(&package_path).map_err(|source| {
                HvigorContextInspectError::Io {
                    path: package_path.clone(),
                    source,
                }
            })?;
            let package: RemoteHspPackage =
                json5::from_str(&content).map_err(|error| HvigorContextInspectError::Parse {
                    path: package_path,
                    message: error.to_string(),
                })?;
            let Ok(version) = Version::parse(&package.version) else {
                continue;
            };
            let files =
                std::fs::read_dir(&directory).map_err(|source| HvigorContextInspectError::Io {
                    path: directory.clone(),
                    source,
                })?;
            let mut hsp_file = None;
            for file in files {
                let file = file.map_err(|source| HvigorContextInspectError::Io {
                    path: directory.clone(),
                    source,
                })?;
                if file.path().extension().and_then(|value| value.to_str()) == Some("hsp") {
                    hsp_file = file.file_name().to_str().map(str::to_string);
                    break;
                }
            }
            let Some(file_name) = hsp_file else {
                continue;
            };
            let should_replace = self
                .by_package_name
                .get(&package.name)
                .is_none_or(|current| version >= current.version);
            if should_replace {
                self.by_package_name
                    .insert(package.name, RemoteHspArtifact { version, file_name });
            }
        }
        Ok(())
    }

    fn dependency_directory_name(package_root: &Path) -> Option<String> {
        let components = package_root.components().collect::<Vec<_>>();
        let index = components
            .iter()
            .rposition(|component| component.as_os_str() == "oh_modules")?;
        (index > 0).then(|| {
            components[index - 1]
                .as_os_str()
                .to_string_lossy()
                .into_owned()
        })
    }
}
