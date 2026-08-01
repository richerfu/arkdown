use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use arkdown_core::ArkdownError;
use arkdown_path::{lexical_normalize, ArtifactPathSerializer};
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
    /// Snapshot of files collected from the current package's `oh-exports`.
    pub oh_export_files: Option<Arc<[PathBuf]>>,
}

#[derive(Debug, Clone)]
pub struct PkgContextDependencyInput {
    pub alias: String,
    pub package_name: String,
    pub version_spec: Option<String>,
    /// Resolved package root captured by the build-session dependency graph.
    pub package_root: Option<PathBuf>,
    /// First entry selected by hvigor-arkts-compose `findNodeEntryFiles()`.
    pub node_entry: Option<String>,
    /// `main`/`types` fallback selected by hvigor's `getEntryPath()`.
    pub main: Option<String>,
    pub kind: PkgContextDependencyKind,
    pub module_name: Option<String>,
    pub bundle_name: Option<String>,
    pub bundle_type: Option<String>,
    /// Snapshot of files collected from the dependency's `oh-exports`.
    pub oh_export_files: Option<Arc<[PathBuf]>>,
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
    #[serde(rename = "oh-exports", skip_serializing_if = "Option::is_none")]
    pub oh_exports: Option<BTreeMap<String, String>>,
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
        let mut context = PkgContextInfoOpt {
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
            oh_exports: None,
        };
        if matches!(
            current.module_kind,
            PkgContextModuleKind::Har | PkgContextModuleKind::Hsp
        ) {
            context.oh_exports = self.oh_exports_context(
                &current.module_root,
                current.oh_export_files.as_deref(),
                &context,
            )?;
        }
        Ok(context)
    }

    fn dependency_context(
        &self,
        current: &PkgContextCurrentInput,
        dependency: &PkgContextDependencyInput,
    ) -> Result<PkgContextInfoOpt, ArkdownError> {
        let mut context = PkgContextInfoOpt {
            package_name: dependency.package_name.clone(),
            bundle_name: self.dependency_bundle_name(current, dependency)?,
            module_name: self.dependency_module_name(dependency)?,
            version: self.dependency_version(dependency)?,
            // `getDependenciesPkgContextInfo` preserves the resolved entry for
            // declared SO dependencies. Only standalone native libraries added
            // by `initLibsSoPkgContextInfo` use `getSoDefaultContextInfo` and
            // therefore have an empty entry path.
            entry_path: self.dependency_entry_path(dependency),
            is_so: dependency.kind == PkgContextDependencyKind::So,
            dependency_alias: if dependency.alias == dependency.package_name {
                String::new()
            } else {
                dependency.alias.clone()
            },
            oh_exports: None,
        };
        if let Some(package_root) = dependency.package_root.as_deref() {
            context.oh_exports = self.oh_exports_context(
                package_root,
                dependency.oh_export_files.as_deref(),
                &context,
            )?;
        }
        Ok(context)
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
            oh_exports: None,
        }
    }

    /// Match
    /// `generate-pkg-context-info.ts#generateOhExportsOhmUrlObj`.
    fn oh_exports_context(
        &self,
        package_root: &Path,
        files: Option<&[PathBuf]>,
        context: &PkgContextInfoOpt,
    ) -> Result<Option<BTreeMap<String, String>>, ArkdownError> {
        let Some(files) = files else {
            return Ok(None);
        };
        let mut oh_exports = BTreeMap::new();
        for file in files {
            let relative = Self::relative_path(package_root, file);
            let relative = ArtifactPathSerializer::path_to_slash_string(&relative)
                .map_err(|error| ArkdownError::Io(std::io::Error::other(error.to_string())))?;
            oh_exports.insert(
                relative.clone(),
                Self::normalized_source_ohm_url(context, &relative),
            );
        }
        Ok(Some(oh_exports))
    }

    fn normalized_source_ohm_url(context: &PkgContextInfoOpt, source_file: &str) -> String {
        let source_file = Self::remove_ohm_suffix(source_file);
        let import_path = if source_file.is_empty() {
            String::new()
        } else {
            format!("{}/{}", context.package_name, source_file)
        };
        format!(
            "@normalized:{}&{}&{}&{}&{}",
            if context.is_so { "Y" } else { "N" },
            context.module_name,
            context.bundle_name,
            import_path,
            context.version
        )
    }

    /// Match `ohmurl-utils.ts#removeSuffix`.
    fn remove_ohm_suffix(path: &str) -> &str {
        const SUFFIXES: [&str; 11] = [
            ".d.ets", ".d.ts", ".d.mjs", ".d.cjs", ".d.js", ".ets", ".ts", ".mjs", ".cjs", ".js",
            ".so",
        ];
        SUFFIXES
            .iter()
            .find_map(|suffix| path.strip_suffix(suffix))
            .unwrap_or(path)
    }

    /// Platform-native equivalent of Node's `path.relative`.
    fn relative_path(from: &Path, to: &Path) -> PathBuf {
        let from = lexical_normalize(from);
        let to = lexical_normalize(to);
        let from_components = from.components().collect::<Vec<_>>();
        let to_components = to.components().collect::<Vec<_>>();
        if !Self::same_path_anchor(&from_components, &to_components) {
            return to;
        }

        let common = from_components
            .iter()
            .zip(&to_components)
            .take_while(|(left, right)| left == right)
            .count();
        let mut relative = PathBuf::new();
        for component in &from_components[common..] {
            if matches!(component, Component::Normal(_)) {
                relative.push("..");
            }
        }
        for component in &to_components[common..] {
            relative.push(component.as_os_str());
        }
        relative
    }

    fn same_path_anchor(left: &[Component<'_>], right: &[Component<'_>]) -> bool {
        match (left.first(), right.first()) {
            (Some(Component::Prefix(left)), Some(Component::Prefix(right))) => left == right,
            (Some(Component::RootDir), Some(Component::RootDir)) => true,
            (Some(Component::Prefix(_)), _) | (_, Some(Component::Prefix(_))) => false,
            (Some(Component::RootDir), _) | (_, Some(Component::RootDir)) => false,
            _ => true,
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
        let Some(package_root) = dependency.package_root.as_deref() else {
            return String::new();
        };
        let package_root = lexical_normalize(package_root);
        let node_entry = dependency.node_entry.as_deref().unwrap_or("index.js");
        if let Some(path) = self.existing_dependency_entry(&package_root, node_entry) {
            return path;
        }
        let fallback = dependency
            .main
            .as_deref()
            .filter(|entry| !entry.is_empty())
            .unwrap_or("index.ets");
        self.existing_dependency_entry(&package_root, fallback)
            .unwrap_or_default()
    }

    /// Match `dependency-manager.ts#getEntryMainFilePath`: only an entry that
    /// exists on disk is written to `pkgContextInfo.json`.
    fn existing_dependency_entry(&self, package_root: &Path, entry: &str) -> Option<String> {
        let path = lexical_normalize(&package_root.join(entry));
        if !path.exists() {
            return None;
        }
        let relative = path.strip_prefix(package_root).ok()?;
        Some(Self::trim_declaration_suffix(
            &ArtifactPathSerializer::rel_to_slash(relative),
        ))
    }

    fn dependency_module_name(
        &self,
        dependency: &PkgContextDependencyInput,
    ) -> Result<String, ArkdownError> {
        if dependency.kind != PkgContextDependencyKind::Hsp {
            return Ok(String::new());
        }
        // `GeneratePkgContextInfo.getDependenciesPkgContextInfo()` uses
        // `dependency.getModuleJsonObj()?.module.name ?? ""`. A malformed or
        // profile-less HSP therefore remains representable in pkgContextInfo;
        // profile validation is owned by the dependency/profile stages.
        Ok(dependency.module_name.clone().unwrap_or_default())
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
        let path = path
            .strip_suffix(".d.ets")
            .or_else(|| path.strip_suffix(".d.ts"))
            .unwrap_or(path);
        ArtifactPathSerializer::rel_to_slash(Path::new(path))
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
            oh_export_files: None,
        }
    }

    #[test]
    fn hap_and_source_har_match_hvigor_runtime_shape() {
        let temp = tempfile::tempdir().unwrap();
        let dependency_root = temp.path().join("audioplayer");
        fs_err::create_dir_all(&dependency_root).unwrap();
        fs_err::write(dependency_root.join("Index.ets"), "export default 1;\n").unwrap();
        let input = PkgContextInfoInput {
            current: current_hap(),
            dependencies: vec![PkgContextDependencyInput {
                alias: "@ohos/audioplayer".to_string(),
                package_name: "audioplayer".to_string(),
                version_spec: Some("file:../../commons/audioplayer".to_string()),
                package_root: Some(dependency_root),
                node_entry: Some("Index.ets".to_string()),
                main: Some("Index.ets".to_string()),
                kind: PkgContextDependencyKind::Har,
                module_name: None,
                bundle_name: None,
                bundle_type: None,
                oh_export_files: None,
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
        let temp = tempfile::tempdir().unwrap();
        let dependency_root = temp.path().join("utils");
        fs_err::create_dir_all(&dependency_root).unwrap();
        fs_err::write(dependency_root.join("Index.d.ets"), "export {};\n").unwrap();
        let input = PkgContextInfoInput {
            current: current_hap(),
            dependencies: vec![PkgContextDependencyInput {
                alias: "utils".to_string(),
                package_name: "utils".to_string(),
                version_spec: Some("1.0.0".to_string()),
                package_root: Some(dependency_root),
                node_entry: Some("Index.d.ets".to_string()),
                main: Some("Index.d.ets".to_string()),
                kind: PkgContextDependencyKind::Other,
                module_name: None,
                bundle_name: None,
                bundle_type: None,
                oh_export_files: None,
            }],
            native_libraries: Vec::new(),
        };

        let entries = PkgContextInfoGenerator::new().generate(&input).unwrap();
        assert_eq!(entries["utils"].dependency_alias, "");
        assert_eq!(entries["utils"].entry_path, "Index");
    }

    #[test]
    fn missing_dependency_entry_is_not_forwarded_to_pkg_context_info() {
        let temp = tempfile::tempdir().unwrap();
        let input = PkgContextInfoInput {
            current: current_hap(),
            dependencies: vec![PkgContextDependencyInput {
                alias: "bc-har".to_string(),
                package_name: "bc-har".to_string(),
                version_spec: Some("1.0.0".to_string()),
                package_root: Some(temp.path().to_path_buf()),
                node_entry: Some("index".to_string()),
                main: Some("index".to_string()),
                kind: PkgContextDependencyKind::Har,
                module_name: None,
                bundle_name: None,
                bundle_type: None,
                oh_export_files: None,
            }],
            native_libraries: Vec::new(),
        };

        let entries = PkgContextInfoGenerator::new().generate(&input).unwrap();
        assert_eq!(entries["bc-har"].entry_path, "");
    }

    #[test]
    fn declared_so_dependency_preserves_its_resolved_entry_path() {
        let temp = tempfile::tempdir().unwrap();
        fs_err::write(temp.path().join("Index.ets"), "export default 1;\n").unwrap();
        let input = PkgContextInfoInput {
            current: current_hap(),
            dependencies: vec![PkgContextDependencyInput {
                alias: "libvad.so".to_string(),
                package_name: "libvad.so".to_string(),
                version_spec: Some("1.0.0".to_string()),
                package_root: Some(temp.path().to_path_buf()),
                node_entry: Some("Index.ets".to_string()),
                main: Some("Index.ets".to_string()),
                kind: PkgContextDependencyKind::So,
                module_name: None,
                bundle_name: None,
                bundle_type: None,
                oh_export_files: None,
            }],
            native_libraries: Vec::new(),
        };

        let entries = PkgContextInfoGenerator::new().generate(&input).unwrap();
        assert!(entries["libvad.so"].is_so);
        assert_eq!(entries["libvad.so"].entry_path, "Index.ets");
    }

    #[test]
    fn current_and_dependency_oh_exports_match_hvigor_normalized_source_urls() {
        let temp = tempfile::tempdir().unwrap();
        let current_root = temp.path().join("current");
        let dependency_root = temp.path().join("dependency");
        fs_err::create_dir_all(current_root.join("src/main/ets")).unwrap();
        fs_err::create_dir_all(dependency_root.join("types")).unwrap();
        let current_export = current_root.join("src/main/ets/Public.ets");
        let dependency_export = dependency_root.join("types/Public.d.ts");
        fs_err::write(&current_export, "export const value = 1;\n").unwrap();
        fs_err::write(&dependency_export, "export declare const value: number;\n").unwrap();

        let input = PkgContextInfoInput {
            current: PkgContextCurrentInput {
                package_name: "@scope/current".to_string(),
                bundle_name: "com.example.shared".to_string(),
                bundle_type: "shared".to_string(),
                module_kind: PkgContextModuleKind::Har,
                module_root: current_root.clone(),
                source_root: current_root.join("src/main/ets"),
                main: Some("Index.ets".to_string()),
                version: "1.2.0".to_string(),
                oh_export_files: Some(Arc::from(vec![current_export])),
            },
            dependencies: vec![PkgContextDependencyInput {
                alias: "public-types".to_string(),
                package_name: "@scope/dependency".to_string(),
                version_spec: Some("file:../dependency".to_string()),
                package_root: Some(dependency_root),
                node_entry: None,
                main: None,
                kind: PkgContextDependencyKind::Har,
                module_name: None,
                bundle_name: None,
                bundle_type: None,
                oh_export_files: Some(Arc::from(vec![dependency_export])),
            }],
            native_libraries: Vec::new(),
        };

        let entries = PkgContextInfoGenerator::new().generate(&input).unwrap();
        assert_eq!(
            entries["@scope/current"].oh_exports.as_ref().unwrap()["src/main/ets/Public.ets"],
            "@normalized:N&&&@scope/current/src/main/ets/Public&1.2.0"
        );
        assert_eq!(
            entries["@scope/dependency"].oh_exports.as_ref().unwrap()["types/Public.d.ts"],
            "@normalized:N&&com.example.shared&@scope/dependency/types/Public&file:../dependency"
        );
    }

    #[test]
    fn present_empty_oh_exports_serializes_as_empty_object() {
        let mut current = current_hap();
        current.module_kind = PkgContextModuleKind::Har;
        current.oh_export_files = Some(Arc::from(Vec::<PathBuf>::new()));
        let entries = PkgContextInfoGenerator::new()
            .generate(&PkgContextInfoInput {
                current,
                dependencies: Vec::new(),
                native_libraries: Vec::new(),
            })
            .unwrap();
        let value = serde_json::to_value(entries).unwrap();

        assert_eq!(value["phone"]["oh-exports"], serde_json::json!({}));
    }
}
