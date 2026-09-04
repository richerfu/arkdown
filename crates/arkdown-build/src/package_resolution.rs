use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use arkdown_path::{canonicalize_or_owned, classify::PathClassifier};
use arkdown_resolver::{
    PackageManifestSnapshot, PackageResolutionPlan, PackageResolutionPlanBuilder, PkgBriefInfo,
};
use arkdown_session::ModuleCompileContext;
use rustc_hash::{FxHashMap, FxHashSet};

/// Selected local-module source roots keyed by canonical package root.
///
/// This is built once from the effective module targets before checker or
/// Rolldown planning. It is the Rust equivalent of hvigor's
/// `getTargetSourceRoots()` result for local dependencies.
#[derive(Default)]
pub struct LocalPackageSourceRootInventory {
    roots: FxHashMap<PathBuf, Arc<[PathBuf]>>,
}

impl LocalPackageSourceRootInventory {
    pub fn insert(&mut self, package_root: &Path, source_roots: impl IntoIterator<Item = PathBuf>) {
        let package_root = canonicalize_or_owned(package_root);
        let mut roots = Vec::new();
        let mut seen = FxHashSet::default();
        for source_root in source_roots {
            let source_root = if source_root.has_root() {
                source_root
            } else {
                package_root.join(source_root)
            };
            let source_root = canonicalize_or_owned(&source_root);
            if seen.insert(source_root.clone()) {
                roots.push(source_root);
            }
        }
        self.roots.insert(package_root, Arc::from(roots));
    }

    fn source_roots(&self, package_root: &Path) -> Option<&[PathBuf]> {
        self.roots.get(package_root).map(AsRef::as_ref)
    }
}

/// Converts the immutable session dependency graph into the single package
/// plan shared by the SDK checker and Rolldown resolver.
pub struct ModulePackageResolutionPlanner<'a> {
    context: &'a ModuleCompileContext,
    local_sources: &'a LocalPackageSourceRootInventory,
}

impl<'a> ModulePackageResolutionPlanner<'a> {
    pub fn new(
        context: &'a ModuleCompileContext,
        local_sources: &'a LocalPackageSourceRootInventory,
    ) -> Self {
        Self {
            context,
            local_sources,
        }
    }

    pub fn build(&self) -> Result<PackageResolutionPlan> {
        let mut builder = PackageResolutionPlanBuilder::new();
        for edge in &self.context.deps.edges {
            let node = self.context.deps.nodes.get(&edge.package_name);
            let package_root = canonicalize_or_owned(&edge.package_root);
            let local_source_roots = self.local_sources.source_roots(&package_root);
            let path_kind = PathClassifier::new(&package_root);
            let local_source = local_source_roots.is_some()
                || !path_kind.is_inside_oh_modules() && !path_kind.is_inside_node_modules();
            let source_roots = if let Some(source_roots) = local_source_roots {
                source_roots.to_vec()
            } else if local_source {
                vec![canonicalize_or_owned(
                    &package_root.join("src").join("main"),
                )]
            } else {
                self.metadata_source_roots(
                    node.and_then(|node| node.package.as_deref()),
                    &package_root,
                )
            };
            builder.insert_dependency(
                &edge.alias,
                PkgBriefInfo {
                    pkg_root: package_root,
                    pkg_name: edge.package_name.clone(),
                    manifest: node
                        .and_then(|node| node.package.as_deref())
                        .map(Self::manifest_snapshot)
                        .unwrap_or_else(|| Arc::new(PackageManifestSnapshot::default())),
                    local_source,
                    source_roots,
                },
            );
        }

        let current = self.context.module.package.as_ref().with_context(|| {
            format!(
                "module '{}' requires package metadata for module resolution",
                self.context.module.name
            )
        })?;
        let current_root = canonicalize_or_owned(&current.root);
        let source_roots = self
            .local_sources
            .source_roots(&current_root)
            .map(<[PathBuf]>::to_vec)
            .unwrap_or_else(|| vec![canonicalize_or_owned(&self.context.module.src_main)]);
        builder.insert_current(PkgBriefInfo {
            pkg_root: current_root,
            pkg_name: current.name.clone(),
            manifest: Self::manifest_snapshot(current),
            local_source: true,
            source_roots,
        });
        Ok(builder.build())
    }

    fn manifest_snapshot(
        package: &arkdown_session::PackageSnapshot,
    ) -> Arc<PackageManifestSnapshot> {
        let metadata = package.package_json.extra.get("metadata");
        Arc::new(PackageManifestSnapshot {
            version: package.version.clone(),
            main: package.package_json.main.clone(),
            types: package.package_json.types.clone(),
            exports: package.package_json.extra.get("exports").cloned(),
            module: package
                .package_json
                .extra
                .get("module")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            jsnext_main: package
                .package_json
                .extra
                .get("jsnext:main")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            byte_code_har: metadata
                .and_then(|metadata| metadata.get("byteCodeHar"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            abc_path: metadata
                .and_then(|metadata| metadata.get("abcPath"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        })
    }

    fn metadata_source_roots(
        &self,
        package: Option<&arkdown_session::PackageSnapshot>,
        package_root: &Path,
    ) -> Vec<PathBuf> {
        package
            .and_then(|package| package.package_json.extra.get("metadata"))
            .and_then(|metadata| metadata.get("sourceRoots"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(|source_root| canonicalize_or_owned(&package_root.join(source_root)))
            .collect()
    }
}
