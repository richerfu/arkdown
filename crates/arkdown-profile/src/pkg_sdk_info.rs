//! Legacy OHM package SDK metadata generation.
//!
//! Hvigor writes `pkgSdkInfo.json` when normalized OHM URLs are disabled and
//! passes it to the SDK package tool. The file records every resolved package's
//! version, dependency kind, and whether it is a direct dependency.

use std::path::Path;

use arkdown_core::ArkdownError;
use arkdown_session::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};
use arkdown_session::module::snapshot::ModuleSnapshot;

/// Generates hvigor-compatible `pkgSdkInfo.json` from immutable session
/// snapshots.
pub struct PkgSdkInfoGenerator<'a> {
    module: &'a ModuleSnapshot,
    dependencies: &'a DependencyGraphSnapshot,
}

impl<'a> PkgSdkInfoGenerator<'a> {
    pub fn new(module: &'a ModuleSnapshot, dependencies: &'a DependencyGraphSnapshot) -> Self {
        Self {
            module,
            dependencies,
        }
    }

    /// Generate the JSON object in hvigor dependency traversal order.
    pub fn generate(&self) -> Result<serde_json::Value, ArkdownError> {
        let package = self.module.package.as_ref().ok_or_else(|| {
            ArkdownError::Io(std::io::Error::other(format!(
                "GeneratePkgSdkInfo requires oh-package.json5 for module '{}'",
                self.module.name
            )))
        })?;
        let mut entries = serde_json::Map::new();
        let mut visited = std::collections::BTreeSet::new();

        for edge in &self.dependencies.edges {
            let Some(node) = self.dependencies.nodes.get(&edge.package_name) else {
                continue;
            };
            if visited.insert(node.package_name.as_str()) {
                entries.insert(node.package_name.clone(), Self::dependency_entry(node));
            }
        }
        for node in self.dependencies.nodes.values() {
            if visited.insert(node.package_name.as_str()) {
                entries.insert(node.package_name.clone(), Self::dependency_entry(node));
            }
        }
        entries.insert(
            package.name.clone(),
            serde_json::json!({
                "version": package.version,
                "type": self.module.module_type.as_str(),
                "isDirectDep": false,
            }),
        );
        Ok(serde_json::Value::Object(entries))
    }

    /// Write minified JSON, matching `GeneratePkgSdkInfo.doTaskAction`.
    pub fn write(&self, path: &Path) -> Result<(), ArkdownError> {
        if let Some(parent) = path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(&self.generate()?)
            .map_err(|error| ArkdownError::Io(std::io::Error::other(error.to_string())))?;
        fs_err::write(path, bytes)?;
        Ok(())
    }

    fn dependency_entry(node: &DependencyNodeSnapshot) -> serde_json::Value {
        let version = node
            .package
            .as_ref()
            .map(|package| package.version.as_str())
            .unwrap_or_default();
        serde_json::json!({
            "version": version,
            "type": Self::dependency_type(node.node_type),
            "isDirectDep": !node.is_transitive,
        })
    }

    fn dependency_type(node_type: DependencyNodeType) -> &'static str {
        match node_type {
            DependencyNodeType::SourceHar | DependencyNodeType::BytecodeHar => "har",
            DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp => "shared",
            DependencyNodeType::NativeSo => "so",
            DependencyNodeType::OhpmSource | DependencyNodeType::Unknown => "other",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use arkdown_session::dependency_graph::snapshot::{
        DependencyEdgeKind, DependencyEdgeSnapshot, DependencyGraphSnapshot,
        DependencyNodeSnapshot, DependencyNodeType,
    };
    use arkdown_session::module::snapshot::{ModuleSnapshot, OhosModuleType};
    use arkdown_session::package::snapshot::PackageSnapshot;

    use super::PkgSdkInfoGenerator;

    fn package(name: &str, version: &str) -> Arc<PackageSnapshot> {
        let package_json =
            json5::from_str(&format!(r#"{{ name: "{name}", version: "{version}" }}"#)).unwrap();
        Arc::new(PackageSnapshot {
            name: name.to_string(),
            version: version.to_string(),
            root: PathBuf::from(format!("/{name}")),
            package_json,
            oh_export_files: None,
        })
    }

    #[test]
    fn generates_hvigor_dependency_types_versions_and_directness() {
        let current_package = package("entry", "1.0.0");
        let module = ModuleSnapshot {
            name: "entry".to_string(),
            root: PathBuf::from("/entry"),
            src_main: PathBuf::from("/entry/src/main"),
            module_json_path: None,
            module_json: None,
            module_json_value: None,
            ohos_test_module_json_value: None,
            module_type: OhosModuleType::Entry,
            package_path: None,
            package: Some(current_package),
            source_roots: Vec::new(),
            resource_roots: Vec::new(),
            related_entry_modules: None,
        };
        let direct_package = package("direct-har", "2.0.0");
        let transitive_package = package("shared-hsp", "3.0.0");
        let mut dependencies = DependencyGraphSnapshot {
            host_module: "entry".to_string(),
            ..DependencyGraphSnapshot::default()
        };
        dependencies.nodes.insert(
            "direct-har".to_string(),
            DependencyNodeSnapshot {
                alias: "direct-har".to_string(),
                package_name: "direct-har".to_string(),
                version_spec: Some("2.0.0".to_string()),
                package_root: Some(PathBuf::from("/direct-har")),
                package: Some(direct_package),
                module_profile: None,
                node_type: DependencyNodeType::BytecodeHar,
                is_runtime_only: false,
                is_dynamic: false,
                is_transitive: false,
                direct_deps: Vec::new(),
            },
        );
        dependencies.nodes.insert(
            "shared-hsp".to_string(),
            DependencyNodeSnapshot {
                alias: "shared-hsp".to_string(),
                package_name: "shared-hsp".to_string(),
                version_spec: Some("3.0.0".to_string()),
                package_root: Some(PathBuf::from("/shared-hsp")),
                package: Some(transitive_package),
                module_profile: None,
                node_type: DependencyNodeType::SharedHsp,
                is_runtime_only: false,
                is_dynamic: false,
                is_transitive: true,
                direct_deps: Vec::new(),
            },
        );
        for package_name in ["direct-har", "shared-hsp"] {
            dependencies.edges.push(DependencyEdgeSnapshot {
                issuer_root: PathBuf::from("/entry"),
                alias: package_name.to_string(),
                version_spec: None,
                package_name: package_name.to_string(),
                package_root: PathBuf::from(format!("/{package_name}")),
                kind: DependencyEdgeKind::Dependency,
                is_transitive: package_name == "shared-hsp",
            });
        }

        let generated = PkgSdkInfoGenerator::new(&module, &dependencies)
            .generate()
            .unwrap();
        assert_eq!(generated["direct-har"]["version"], "2.0.0");
        assert_eq!(generated["direct-har"]["type"], "har");
        assert_eq!(generated["direct-har"]["isDirectDep"], true);
        assert_eq!(generated["shared-hsp"]["type"], "shared");
        assert_eq!(generated["shared-hsp"]["isDirectDep"], false);
        assert_eq!(generated["entry"]["type"], "entry");
        assert_eq!(generated["entry"]["isDirectDep"], false);
    }
}
