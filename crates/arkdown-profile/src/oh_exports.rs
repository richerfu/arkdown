//! hvigor-compatible `oh-exports` validation and Loader integration.

use std::path::Path;

use arkdown_core::ArkdownError;
use arkdown_path::lexical_normalize;
use arkdown_session::module::snapshot::OhosModuleType;
use arkdown_session::package::snapshot::PackageSnapshot;
use rustc_hash::FxHashSet;

use crate::LoaderJson;

/// Applies the current package's snapshotted `oh-exports` contract.
pub struct OhExportsProcessor;

impl OhExportsProcessor {
    pub fn new() -> Self {
        Self
    }

    /// Validate the declaration and, for HAR/HSP modules, add the collected
    /// files to `loader.json` and its compile-entry inventory.
    ///
    /// Upstream correspondence:
    /// `generate-loader-json.ts#checkOhExports`,
    /// `#addOhExports2CompileEntry`, and `#writeOhExportsToLoaderJson`.
    pub fn process(
        &self,
        package: &PackageSnapshot,
        module_type: &OhosModuleType,
        loader: &mut LoaderJson,
    ) -> Result<(), ArkdownError> {
        if package.package_json.oh_exports.is_none() {
            return Ok(());
        }
        self.validate(package, loader.use_normalized_ohm_url)?;
        if matches!(module_type, OhosModuleType::Har | OhosModuleType::Shared) {
            loader.apply_oh_exports_files(
                package
                    .oh_export_files
                    .as_deref()
                    .expect("present oh-exports must have a collected snapshot"),
            );
        }
        Ok(())
    }

    fn validate(
        &self,
        package: &PackageSnapshot,
        use_normalized_ohm_url: bool,
    ) -> Result<(), ArkdownError> {
        let declarations = package
            .package_json
            .oh_exports
            .as_deref()
            .expect("validation requires a present oh-exports field");
        if !use_normalized_ohm_url {
            return Err(ArkdownError::Bundler(format!(
                "OH_EXPORTS_NOT_SUPPORTED_WHEN_USENORMALIZEDOHMURL_IS_NOT_TRUE: {}",
                package.root.join("oh-package.json5").display()
            )));
        }

        let root = lexical_normalize(&package.root);
        let mut invalid_paths = Vec::new();
        let mut unsupported_files = Vec::new();
        let mut duplicate_paths = Vec::new();
        let mut resolved_paths = FxHashSet::default();

        for declaration in declarations {
            let declared_path = Path::new(declaration);
            let resolved = lexical_normalize(&root.join(declared_path));
            let is_strict_descendant = resolved != root && resolved.starts_with(&root);
            if declaration.is_empty()
                || declared_path.has_root()
                || !is_strict_descendant
                || !resolved.exists()
            {
                invalid_paths.push(declaration.clone());
                continue;
            }
            if !resolved_paths.insert(resolved.clone()) {
                duplicate_paths.push(declaration.clone());
            }
            if resolved.is_file() && !Self::is_supported_source_file(&resolved) {
                unsupported_files.push(declaration.clone());
            }
        }

        let manifest_path = package.root.join("oh-package.json5");
        if !invalid_paths.is_empty() {
            return Err(ArkdownError::Bundler(format!(
                "OH_EXPORTS_PATH_IS_INVALID: {:?}, {}",
                invalid_paths,
                manifest_path.display()
            )));
        }
        if !unsupported_files.is_empty() {
            return Err(ArkdownError::Bundler(format!(
                "OH_EXPORTS_NOT_SUPPORTED_FILE: {:?}, {}",
                unsupported_files,
                manifest_path.display()
            )));
        }
        if !duplicate_paths.is_empty() {
            tracing::warn!(
                duplicates = ?duplicate_paths,
                manifest = %manifest_path.display(),
                "Duplicate path in 'oh-exports'"
            );
        }
        Ok(())
    }

    fn is_supported_source_file(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts" | "js")
        )
    }
}

impl Default for OhExportsProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{OhExportsProcessor, OhosModuleType};
    use crate::LoaderJson;
    use arkdown_session::package::snapshot::PackageSnapshot;

    fn loader(module_root: &std::path::Path, use_normalized_ohm_url: bool) -> LoaderJson {
        LoaderJson {
            module_path_map: Default::default(),
            compile_mode: "esmodule".to_string(),
            project_root_path: module_root.display().to_string(),
            node_modules_path: String::new(),
            byte_code_har_info: Default::default(),
            declaration_entry: Vec::new(),
            module_name: "library".to_string(),
            hsp_name_ohm_map: Default::default(),
            har_name_ohm_map: Default::default(),
            package_manager_type: "ohpm".to_string(),
            hvigor_compile_entry: Vec::new(),
            other_compile_files: Vec::new(),
            dynamic_import_lib_info: Default::default(),
            router_map: Vec::new(),
            hsp_resources_map: Default::default(),
            update_version_info: Default::default(),
            customized_har: false,
            byte_code_har: None,
            pkg_name2_other_compile_files: None,
            an_build_out_put: None,
            an_build_mode: None,
            ap_path: None,
            build_config_path: None,
            patch_config: None,
            non_entry_importees: Vec::new(),
            compiled_importees: Vec::new(),
            compile_entry: Vec::new(),
            form_entry_sources: Vec::new(),
            runtime_entry_sources: Vec::new(),
            page_entry_sources: Vec::new(),
            module_root_path: crate::loader_json::to_absolute_path_string(
                &module_root.join("src/main"),
            ),
            cache_path: String::new(),
            ace_module_build: String::new(),
            build_mode: "debug".to_string(),
            support_chunks: false,
            use_normalized_ohm_url,
            api_version: 12,
            fallback_an_build: false,
            pkg_name_to_pkg_brief_info: Default::default(),
            workers: Vec::new(),
            source_roots: Vec::new(),
            bundle_dependencies: Vec::new(),
            global_dependencies: Vec::new(),
            compiler_types: Vec::new(),
            compiler_type_declaration_inputs: Vec::new(),
            dependency_pkg_version: Vec::new(),
            extra_importees: Vec::new(),
            compiled_package_names: Vec::new(),
            oh_exports: Vec::new(),
        }
    }

    #[test]
    fn present_empty_declaration_still_requires_normalized_ohm_urls() {
        let temp = tempfile::tempdir().unwrap();
        fs_err::write(
            temp.path().join("oh-package.json5"),
            r#"{ name: "library", "oh-exports": [] }"#,
        )
        .unwrap();
        let package = PackageSnapshot::from_root(temp.path())
            .unwrap()
            .expect("package");

        let error = OhExportsProcessor::new()
            .validate(&package, false)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("OH_EXPORTS_NOT_SUPPORTED_WHEN_USENORMALIZEDOHMURL_IS_NOT_TRUE"));
    }

    #[test]
    fn rejects_missing_outside_absolute_and_unsupported_direct_files() {
        let temp = tempfile::tempdir().unwrap();
        let package_root = temp.path().join("package");
        fs_err::create_dir_all(&package_root).unwrap();
        let outside = temp.path().join("outside.ets");
        fs_err::write(&outside, "").unwrap();
        fs_err::write(package_root.join("unsupported.json"), "{}").unwrap();
        let manifest = serde_json::json!({
            "name": "library",
            "oh-exports": ["", "../outside.ets", "missing.ets", outside]
        });
        fs_err::write(
            package_root.join("oh-package.json5"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let package = PackageSnapshot::from_root(&package_root)
            .unwrap()
            .expect("package");

        let error = OhExportsProcessor::new()
            .validate(&package, true)
            .unwrap_err();
        assert!(error.to_string().contains("OH_EXPORTS_PATH_IS_INVALID"));

        fs_err::write(
            package_root.join("oh-package.json5"),
            r#"{ name: "library", "oh-exports": ["unsupported.json"] }"#,
        )
        .unwrap();
        let package = PackageSnapshot::from_root(&package_root)
            .unwrap()
            .expect("package");
        let error = OhExportsProcessor::new()
            .validate(&package, true)
            .unwrap_err();
        assert!(error.to_string().contains("OH_EXPORTS_NOT_SUPPORTED_FILE"));
    }

    #[test]
    fn har_loader_keeps_declared_files_but_excludes_declarations_from_compile_entries() {
        let temp = tempfile::tempdir().unwrap();
        let export_root = temp.path().join("src/main/ets/public");
        fs_err::create_dir_all(&export_root).unwrap();
        let source = export_root.join("Public.ets");
        let declaration = export_root.join("Public.d.ts");
        fs_err::write(&source, "export const value = 1;\n").unwrap();
        fs_err::write(&declaration, "export declare const value: number;\n").unwrap();
        fs_err::write(
            temp.path().join("oh-package.json5"),
            r#"{ name: "library", "oh-exports": ["src/main/ets/public"] }"#,
        )
        .unwrap();
        let package = PackageSnapshot::from_root(temp.path())
            .unwrap()
            .expect("package");
        let mut loader = loader(temp.path(), true);

        OhExportsProcessor::new()
            .process(&package, &OhosModuleType::Har, &mut loader)
            .unwrap();

        assert_eq!(loader.oh_exports.len(), 2);
        assert_eq!(loader.hvigor_compile_entry.len(), 1);
        assert!(loader.hvigor_compile_entry[0].ends_with("/Public.ets"));
        assert_eq!(loader.compile_entry, vec!["./ets/public/Public.ets"]);
        let value = serde_json::to_value(loader).unwrap();
        assert_eq!(value["oh-exports"].as_array().unwrap().len(), 2);
    }
}
