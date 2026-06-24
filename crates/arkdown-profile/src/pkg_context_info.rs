use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arkdown_core::ArkdownError;
use arkdown_path::ArtifactPathSerializer;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgContextModuleKind {
    Hap,
    Hsp,
    Har,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgContextDependencyKind {
    Har,
    Hsp,
    Other,
    So,
}

#[derive(Debug, Clone)]
pub struct PkgContextCurrentInput {
    pub package_name: String,
    pub bundle_name: String,
    pub bundle_type: String,
    pub module_kind: PkgContextModuleKind,
    pub module_root: PathBuf,
    pub source_root: PathBuf,
    pub main: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct PkgContextDependencyInput {
    pub alias: String,
    pub package_name: String,
    pub version_spec: Option<String>,
    pub main: Option<String>,
    pub kind: PkgContextDependencyKind,
    pub module_name: Option<String>,
    pub bundle_name: Option<String>,
    pub bundle_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PkgContextInfoInput {
    pub current: PkgContextCurrentInput,
    pub dependencies: Vec<PkgContextDependencyInput>,
    pub native_libraries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PkgContextInfoOpt {
    pub package_name: String,
    pub bundle_name: String,
    pub module_name: String,
    pub version: String,
    pub entry_path: String,
    #[serde(rename = "isSO")]
    pub is_so: bool,
    pub dependency_alias: String,
    #[serde(rename = "oh-exports", skip_serializing_if = "BTreeMap::is_empty")]
    pub oh_exports: BTreeMap<String, String>,
}

pub struct PkgContextInfoGenerator;

impl PkgContextInfoGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate(
        &self,
        input: &PkgContextInfoInput,
    ) -> Result<BTreeMap<String, PkgContextInfoOpt>, ArkdownError> {
        let mut entries = BTreeMap::new();
        for dependency in &input.dependencies {
            let info = self.dependency_context(&input.current, dependency)?;
            entries.insert(dependency.package_name.clone(), info);
        }
        entries.insert(
            input.current.package_name.clone(),
            self.current_context(&input.current)?,
        );
        for so_name in &input.native_libraries {
            entries
                .entry(so_name.clone())
                .or_insert_with(|| self.so_context(so_name, &input.current));
        }
        Ok(entries)
    }

    pub fn write(
        &self,
        input: &PkgContextInfoInput,
        output_path: &Path,
    ) -> Result<(), ArkdownError> {
        if let Some(parent) = output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        let entries = self.generate(input)?;
        let json = serde_json::to_string(&entries)
            .map_err(|error| ArkdownError::Io(std::io::Error::other(error.to_string())))?;
        fs_err::write(output_path, json)?;
        Ok(())
    }

    fn current_context(
        &self,
        current: &PkgContextCurrentInput,
    ) -> Result<PkgContextInfoOpt, ArkdownError> {
        Ok(PkgContextInfoOpt {
            package_name: current.package_name.clone(),
            bundle_name: self.current_module_bundle_name(current),
            module_name: String::new(),
            version: if current.module_kind == PkgContextModuleKind::Har {
                current.version.clone()
            } else {
                String::new()
            },
            entry_path: self.current_entry_path(current)?,
            is_so: false,
            dependency_alias: String::new(),
            oh_exports: BTreeMap::new(),
        })
    }

    fn dependency_context(
        &self,
        current: &PkgContextCurrentInput,
        dependency: &PkgContextDependencyInput,
    ) -> Result<PkgContextInfoOpt, ArkdownError> {
        Ok(PkgContextInfoOpt {
            package_name: dependency.package_name.clone(),
            bundle_name: self.dependency_bundle_name(current, dependency)?,
            module_name: self.dependency_module_name(dependency)?,
            version: self.dependency_version(dependency)?,
            entry_path: self.dependency_entry_path(dependency),
            is_so: dependency.kind == PkgContextDependencyKind::So,
            dependency_alias: if dependency.alias == dependency.package_name {
                String::new()
            } else {
                dependency.alias.clone()
            },
            oh_exports: BTreeMap::new(),
        })
    }

    fn so_context(&self, so_name: &str, current: &PkgContextCurrentInput) -> PkgContextInfoOpt {
        PkgContextInfoOpt {
            package_name: so_name.to_string(),
            bundle_name: self.current_module_bundle_name(current),
            module_name: String::new(),
            version: String::new(),
            entry_path: String::new(),
            is_so: true,
            dependency_alias: String::new(),
            oh_exports: BTreeMap::new(),
        }
    }

    fn current_entry_path(&self, current: &PkgContextCurrentInput) -> Result<String, ArkdownError> {
        match current.module_kind {
            PkgContextModuleKind::Hap => {
                let rel = current
                    .source_root
                    .strip_prefix(&current.module_root)
                    .map_err(|error| ArkdownError::Io(std::io::Error::other(error.to_string())))?;
                let mut path = ArtifactPathSerializer::rel_to_slash(rel);
                if !path.is_empty() && !path.ends_with('/') {
                    path.push('/');
                }
                Ok(path)
            }
            PkgContextModuleKind::Har => Ok(current
                .main
                .as_deref()
                .map(Self::trim_declaration_suffix)
                .unwrap_or_default()),
            PkgContextModuleKind::Hsp => Ok(String::new()),
        }
    }

    fn dependency_entry_path(&self, dependency: &PkgContextDependencyInput) -> String {
        dependency
            .main
            .as_deref()
            .map(Self::trim_declaration_suffix)
            .unwrap_or_default()
    }

    fn dependency_module_name(
        &self,
        dependency: &PkgContextDependencyInput,
    ) -> Result<String, ArkdownError> {
        if dependency.kind != PkgContextDependencyKind::Hsp {
            return Ok(String::new());
        }
        dependency.module_name.clone().ok_or_else(|| {
            ArkdownError::Io(std::io::Error::other(format!(
                "GeneratePkgContextInfo requires moduleName for HSP dependency '{}'",
                dependency.package_name
            )))
        })
    }

    fn dependency_version(
        &self,
        dependency: &PkgContextDependencyInput,
    ) -> Result<String, ArkdownError> {
        match dependency.kind {
            PkgContextDependencyKind::Har | PkgContextDependencyKind::Other => {
                dependency.version_spec.clone().ok_or_else(|| {
                    ArkdownError::Io(std::io::Error::other(format!(
                        "GeneratePkgContextInfo requires dependency version spec for '{}'",
                        dependency.package_name
                    )))
                })
            }
            PkgContextDependencyKind::Hsp | PkgContextDependencyKind::So => Ok(String::new()),
        }
    }

    fn dependency_bundle_name(
        &self,
        current: &PkgContextCurrentInput,
        dependency: &PkgContextDependencyInput,
    ) -> Result<String, ArkdownError> {
        let current_shared = Self::is_bundle_type_shared_or_app_plugin(&current.bundle_type);
        if current_shared {
            if dependency.kind != PkgContextDependencyKind::Hsp {
                return Ok(current.bundle_name.clone());
            }
            if self.dependency_is_common_app_hsp(dependency) {
                return Ok(dependency.bundle_name.clone().unwrap_or_default());
            }
            return Ok(current.bundle_name.clone());
        }

        if self.dependency_is_common_app_hsp(dependency) {
            let bundle_name = dependency.bundle_name.clone().unwrap_or_default();
            if bundle_name != current.bundle_name {
                return Ok(bundle_name);
            }
        }
        Ok(String::new())
    }

    fn current_module_bundle_name(&self, current: &PkgContextCurrentInput) -> String {
        if current.module_kind == PkgContextModuleKind::Hsp
            && Self::is_bundle_type_shared_or_app_plugin(&current.bundle_type)
        {
            current.bundle_name.clone()
        } else {
            String::new()
        }
    }

    fn dependency_is_common_app_hsp(&self, dependency: &PkgContextDependencyInput) -> bool {
        dependency.kind == PkgContextDependencyKind::Hsp
            && dependency
                .bundle_type
                .as_deref()
                .is_some_and(Self::is_bundle_type_shared_or_app_plugin)
    }

    fn is_bundle_type_shared_or_app_plugin(bundle_type: &str) -> bool {
        bundle_type == "shared" || bundle_type == "appPlugin"
    }

    fn trim_declaration_suffix(path: &str) -> String {
        path.strip_suffix(".d.ets")
            .or_else(|| path.strip_suffix(".d.ts"))
            .unwrap_or(path)
            .replace('\\', "/")
    }
}

impl Default for PkgContextInfoGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_hap() -> PkgContextCurrentInput {
        PkgContextCurrentInput {
            package_name: "phone".to_string(),
            bundle_name: "com.example.hmosworld".to_string(),
            bundle_type: "app".to_string(),
            module_kind: PkgContextModuleKind::Hap,
            module_root: PathBuf::from("/project/products/phone"),
            source_root: PathBuf::from("/project/products/phone/src/main"),
            main: Some(String::new()),
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn hap_and_source_har_match_hvigor_runtime_shape() {
        let input = PkgContextInfoInput {
            current: current_hap(),
            dependencies: vec![PkgContextDependencyInput {
                alias: "@ohos/audioplayer".to_string(),
                package_name: "audioplayer".to_string(),
                version_spec: Some("file:../../commons/audioplayer".to_string()),
                main: Some("Index.ets".to_string()),
                kind: PkgContextDependencyKind::Har,
                module_name: None,
                bundle_name: None,
                bundle_type: None,
            }],
            native_libraries: vec!["libagccrypto.so".to_string()],
        };

        let entries = PkgContextInfoGenerator::new().generate(&input).unwrap();
        assert_eq!(entries["phone"].bundle_name, "");
        assert_eq!(entries["phone"].module_name, "");
        assert_eq!(entries["phone"].entry_path, "src/main/");

        let har = &entries["audioplayer"];
        assert_eq!(har.package_name, "audioplayer");
        assert_eq!(har.bundle_name, "");
        assert_eq!(har.module_name, "");
        assert_eq!(har.version, "file:../../commons/audioplayer");
        assert_eq!(har.entry_path, "Index.ets");
        assert_eq!(har.dependency_alias, "@ohos/audioplayer");

        let so = &entries["libagccrypto.so"];
        assert!(so.is_so);
        assert_eq!(so.bundle_name, "");
    }

    #[test]
    fn dependency_alias_is_empty_when_alias_matches_package_name() {
        let input = PkgContextInfoInput {
            current: current_hap(),
            dependencies: vec![PkgContextDependencyInput {
                alias: "utils".to_string(),
                package_name: "utils".to_string(),
                version_spec: Some("1.0.0".to_string()),
                main: Some("Index.d.ets".to_string()),
                kind: PkgContextDependencyKind::Other,
                module_name: None,
                bundle_name: None,
                bundle_type: None,
            }],
            native_libraries: Vec::new(),
        };

        let entries = PkgContextInfoGenerator::new().generate(&input).unwrap();
        assert_eq!(entries["utils"].dependency_alias, "");
        assert_eq!(entries["utils"].entry_path, "Index");
    }
}
