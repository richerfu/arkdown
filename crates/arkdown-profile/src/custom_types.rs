//! Effective `buildOption.arkOptions.types` resolution.
//!
//! Upstream ownership:
//! - `AbstractPreBuild.checkCustomTypesSupport`
//! - `AbstractPreBuild.checkCustomTypesIsValid`
//! - `build-profile-utils.getCustomTypePathIfExists`
//! - `AbstractCompileNode.collectCustomTypes`

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use arkdown_path::canonicalize_or_owned;
use arkdown_session::DependencyGraphSnapshot;
use rustc_hash::FxHashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CustomTypeError {
    #[error(
        "custom types require compile API 10 or later, but API {compile_api_version} is selected in {profile_path}"
    )]
    UnsupportedApi {
        compile_api_version: u32,
        profile_path: PathBuf,
    },
    #[error(
        "buildOption.arkOptions.types requires ArkTS sources in module '{module_name}' ({profile_path})"
    )]
    RequiresArkTs {
        module_name: String,
        profile_path: PathBuf,
    },
    #[error(
        "custom type '{configured}' redundantly includes '{suffix}'; remove the declaration suffix in {profile_path}"
    )]
    RedundantDeclarationSuffix {
        configured: String,
        suffix: &'static str,
        profile_path: PathBuf,
    },
    #[error(
        "custom type '{configured}' has an invalid extension; configure a dependency name or an extensionless declaration path in {profile_path}"
    )]
    InvalidExtension {
        configured: String,
        profile_path: PathBuf,
    },
    #[error(
        "custom type '{configured}' does not resolve to a dependency, <path>.d.ets, <path>.d.ts, <path>/index.d.ets, or <path>/index.d.ts ({profile_path})"
    )]
    NotFound {
        configured: String,
        profile_path: PathBuf,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomTypeInventory {
    /// Raw compilerTypes values in configured order.
    pub compiler_types: Vec<String>,
    /// Exact declaration files or dependency roots loaded into the OXC
    /// transform snapshot and tracked by the compile-task cache.
    pub declaration_inputs: Vec<PathBuf>,
}

/// Dependency-owned custom type declarations collected by
/// `ArkCompile.getDependencyCustomTypes()`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyCustomTypeInput {
    pub package_name: String,
    pub package_root: PathBuf,
    pub configured_types: Vec<String>,
    pub declared_dependencies: BTreeSet<String>,
}

pub struct CustomTypeResolver<'a> {
    module_name: &'a str,
    module_root: &'a Path,
    profile_path: &'a Path,
    compile_api_version: u32,
    has_arkts_sources: bool,
    dependencies: &'a DependencyGraphSnapshot,
}

impl<'a> CustomTypeResolver<'a> {
    pub fn new(
        module_name: &'a str,
        module_root: &'a Path,
        profile_path: &'a Path,
        compile_api_version: u32,
        has_arkts_sources: bool,
        dependencies: &'a DependencyGraphSnapshot,
    ) -> Self {
        Self {
            module_name,
            module_root,
            profile_path,
            compile_api_version,
            has_arkts_sources,
            dependencies,
        }
    }

    pub fn resolve(
        &self,
        configured_types: &[String],
    ) -> Result<CustomTypeInventory, CustomTypeError> {
        self.resolve_with_dependencies(configured_types, &[])
    }

    /// Resolve host types and the dependency types enabled by
    /// `ohos.dependencies.types.enable` in one deterministic inventory.
    pub fn resolve_with_dependencies(
        &self,
        configured_types: &[String],
        dependency_types: &[DependencyCustomTypeInput],
    ) -> Result<CustomTypeInventory, CustomTypeError> {
        if configured_types.is_empty()
            && dependency_types
                .iter()
                .all(|dependency| dependency.configured_types.is_empty())
        {
            return Ok(CustomTypeInventory::default());
        }
        if self.compile_api_version < 10 {
            return Err(CustomTypeError::UnsupportedApi {
                compile_api_version: self.compile_api_version,
                profile_path: self.profile_path.to_path_buf(),
            });
        }
        if !self.has_arkts_sources {
            return Err(CustomTypeError::RequiresArkTs {
                module_name: self.module_name.to_string(),
                profile_path: self.profile_path.to_path_buf(),
            });
        }

        let mut declaration_inputs = Vec::with_capacity(configured_types.len());
        let mut seen_inputs = FxHashSet::default();
        for configured in configured_types {
            let declaration_input = if let Some(input) = self.dependency_input(configured.as_str())
            {
                canonicalize_or_owned(&input)
            } else {
                canonicalize_or_owned(&self.resolve_declaration_path(configured)?)
            };
            if seen_inputs.insert(declaration_input.clone()) {
                declaration_inputs.push(declaration_input);
            }
        }

        let mut compiler_types = configured_types.to_vec();
        for dependency in dependency_types {
            for configured in &dependency.configured_types {
                if dependency.declared_dependencies.contains(configured)
                    || dependency
                        .declared_dependencies
                        .contains(&format!("@types/{configured}"))
                {
                    if !configured_types.contains(configured) {
                        tracing::warn!(
                            dependency = %dependency.package_name,
                            custom_type = configured,
                            "dependency custom type names another dependency; configure it on the host module"
                        );
                    }
                    continue;
                }
                let Some(declaration_input) =
                    Self::find_declaration_path(&dependency.package_root, configured)
                else {
                    tracing::warn!(
                        dependency = %dependency.package_name,
                        custom_type = configured,
                        "dependency custom type cannot be resolved"
                    );
                    continue;
                };
                let compiler_type = {
                    let configured_path = Path::new(configured);
                    if configured_path.has_root() {
                        configured_path.to_path_buf()
                    } else {
                        dependency.package_root.join(configured_path)
                    }
                };
                let compiler_type = canonicalize_or_owned(&compiler_type)
                    .to_string_lossy()
                    .into_owned();
                if !compiler_types.contains(&compiler_type) {
                    compiler_types.push(compiler_type);
                }
                let declaration_input = canonicalize_or_owned(&declaration_input);
                if seen_inputs.insert(declaration_input.clone()) {
                    declaration_inputs.push(declaration_input);
                }
            }
        }

        Ok(CustomTypeInventory {
            compiler_types,
            declaration_inputs,
        })
    }

    fn dependency_input(&self, configured: &str) -> Option<PathBuf> {
        self.dependency_input_by_name(configured)
            .or_else(|| self.dependency_input_by_name(&format!("@types/{configured}")))
    }

    fn dependency_input_by_name(&self, name: &str) -> Option<PathBuf> {
        self.dependencies
            .nodes
            .values()
            .find(|node| node.alias == name || node.package_name == name)
            .and_then(|node| {
                let root = node.package_root.as_ref()?;
                let manifest_entry = node
                    .package
                    .as_deref()
                    .and_then(|package| package.package_json.types.as_deref())
                    .map(|entry| root.join(entry))
                    .filter(|entry| entry.is_file());
                manifest_entry
                    .or_else(|| {
                        ["index.d.ets", "index.d.ts", "Index.d.ets", "Index.d.ts"]
                            .into_iter()
                            .map(|entry| root.join(entry))
                            .find(|entry| entry.is_file())
                    })
                    .or_else(|| Some(root.clone()))
            })
    }

    fn resolve_declaration_path(&self, configured: &str) -> Result<PathBuf, CustomTypeError> {
        let base = {
            let configured_path = Path::new(configured);
            if configured_path.has_root() {
                configured_path.to_path_buf()
            } else {
                self.module_root.join(configured_path)
            }
        };

        if base.extension().is_some() && base.exists() {
            for suffix in [".d.ets", ".d.ts"] {
                if configured.ends_with(suffix) {
                    return Err(CustomTypeError::RedundantDeclarationSuffix {
                        configured: configured.to_string(),
                        suffix,
                        profile_path: self.profile_path.to_path_buf(),
                    });
                }
            }
            return Err(CustomTypeError::InvalidExtension {
                configured: configured.to_string(),
                profile_path: self.profile_path.to_path_buf(),
            });
        }

        if let Some(candidate) = Self::find_declaration_path(self.module_root, configured) {
            return Ok(candidate);
        }

        Err(CustomTypeError::NotFound {
            configured: configured.to_string(),
            profile_path: self.profile_path.to_path_buf(),
        })
    }

    fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
        let mut value = path.as_os_str().to_os_string();
        value.push(suffix);
        PathBuf::from(value)
    }

    fn find_declaration_path(root: &Path, configured: &str) -> Option<PathBuf> {
        let configured_path = Path::new(configured);
        let base = if configured_path.has_root() {
            configured_path.to_path_buf()
        } else {
            root.join(configured_path)
        };
        [
            Self::append_suffix(&base, ".d.ets"),
            Self::append_suffix(&base, ".d.ts"),
            base.join("index.d.ets"),
            base.join("index.d.ts"),
        ]
        .into_iter()
        .find(|candidate| candidate.exists())
    }
}

#[cfg(test)]
mod tests {
    use arkdown_session::dependency_graph::snapshot::{DependencyNodeSnapshot, DependencyNodeType};

    use super::*;

    fn dependency_graph(alias: &str, package_name: &str, root: &Path) -> DependencyGraphSnapshot {
        let mut graph = DependencyGraphSnapshot::default();
        graph.nodes.insert(
            package_name.to_string(),
            DependencyNodeSnapshot {
                alias: alias.to_string(),
                package_name: package_name.to_string(),
                version_spec: None,
                package_root: Some(root.to_path_buf()),
                package: None,
                module_profile: None,
                node_type: DependencyNodeType::OhpmSource,
                is_runtime_only: false,
                is_dynamic: false,
                is_transitive: false,
                direct_deps: Vec::new(),
            },
        );
        graph
    }

    #[test]
    fn resolves_hvigor_file_candidates_and_dependency_aliases_in_configured_order() {
        let temp = tempfile::tempdir().unwrap();
        let module_root = temp.path().join("entry");
        let dependency_root = temp.path().join("types-package");
        std::fs::create_dir_all(module_root.join("types/runtime")).unwrap();
        std::fs::create_dir_all(&dependency_root).unwrap();
        std::fs::write(
            module_root.join("types/runtime/index.d.ets"),
            "declare interface Runtime {}",
        )
        .unwrap();
        let graph = dependency_graph("runtime-package", "@scope/runtime-types", &dependency_root);
        let inventory = CustomTypeResolver::new(
            "entry",
            &module_root,
            &temp.path().join("build-profile.json5"),
            12,
            true,
            &graph,
        )
        .resolve(&["./types/runtime".to_string(), "runtime-package".to_string()])
        .unwrap();

        assert_eq!(
            inventory.compiler_types,
            vec!["./types/runtime", "runtime-package"]
        );
        assert_eq!(
            inventory.declaration_inputs,
            vec![
                canonicalize_or_owned(&module_root.join("types/runtime/index.d.ets")),
                canonicalize_or_owned(&dependency_root),
            ]
        );
    }

    #[test]
    fn rejects_redundant_declaration_suffix_like_abstract_pre_build() {
        let temp = tempfile::tempdir().unwrap();
        let declaration = temp.path().join("runtime.d.ts");
        std::fs::write(&declaration, "declare interface Runtime {}").unwrap();
        let graph = DependencyGraphSnapshot::default();
        let error = CustomTypeResolver::new(
            "entry",
            temp.path(),
            &temp.path().join("build-profile.json5"),
            12,
            true,
            &graph,
        )
        .resolve(&["runtime.d.ts".to_string()])
        .unwrap_err();

        assert!(matches!(
            error,
            CustomTypeError::RedundantDeclarationSuffix {
                suffix: ".d.ts",
                ..
            }
        ));
    }

    #[test]
    fn rejects_custom_types_before_api_10() {
        let temp = tempfile::tempdir().unwrap();
        let graph = DependencyGraphSnapshot::default();
        let error = CustomTypeResolver::new(
            "entry",
            temp.path(),
            &temp.path().join("build-profile.json5"),
            9,
            true,
            &graph,
        )
        .resolve(&["runtime".to_string()])
        .unwrap_err();

        assert!(matches!(
            error,
            CustomTypeError::UnsupportedApi {
                compile_api_version: 9,
                ..
            }
        ));
    }

    #[test]
    fn appends_hvigor_dependency_custom_types_and_skips_dependency_names() {
        let temp = tempfile::tempdir().unwrap();
        let module_root = temp.path().join("entry");
        let dependency_root = temp.path().join("source-har");
        std::fs::create_dir_all(&module_root).unwrap();
        std::fs::create_dir_all(dependency_root.join("types/runtime")).unwrap();
        std::fs::write(
            dependency_root.join("types/runtime/index.d.ets"),
            "declare interface Runtime {}",
        )
        .unwrap();
        let graph = DependencyGraphSnapshot::default();
        let inventory = CustomTypeResolver::new(
            "entry",
            &module_root,
            &temp.path().join("build-profile.json5"),
            12,
            true,
            &graph,
        )
        .resolve_with_dependencies(
            &[],
            &[DependencyCustomTypeInput {
                package_name: "source-har".to_string(),
                package_root: dependency_root.clone(),
                configured_types: vec!["./types/runtime".to_string(), "nested-types".to_string()],
                declared_dependencies: BTreeSet::from(["nested-types".to_string()]),
            }],
        )
        .unwrap();

        assert_eq!(
            inventory.compiler_types,
            [
                canonicalize_or_owned(&dependency_root.join("types/runtime"))
                    .to_string_lossy()
                    .into_owned()
            ]
        );
        assert_eq!(
            inventory.declaration_inputs,
            [canonicalize_or_owned(
                &dependency_root.join("types/runtime/index.d.ets")
            )]
        );
    }
}
