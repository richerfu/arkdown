use super::{NativeCheckError, NativeCompilerOptions};
use arkdown_path::ArtifactPathSerializer;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

pub(super) struct CompilerPaths;
impl CompilerPaths {
    pub(super) fn one(path: &Path) -> Result<String, NativeCheckError> {
        Ok(ArtifactPathSerializer::path_to_slash_string(path)?)
    }
    pub(super) fn many(paths: &[PathBuf]) -> Result<Vec<String>, NativeCheckError> {
        paths.iter().map(|p| Self::one(p)).collect()
    }
    fn map(
        paths: &BTreeMap<String, Vec<PathBuf>>,
    ) -> Result<tsrs_core::collections::OrderedMap<String, Vec<String>>, NativeCheckError> {
        paths
            .iter()
            .map(|(key, values)| Ok((key.clone(), Self::many(values)?)))
            .collect()
    }
}
impl NativeCompilerOptions {
    pub(super) fn to_tsrs(
        &self,
        executor: Option<Arc<dyn tsrs_core::OhSdkPluginExecutor>>,
    ) -> Result<tsrs_core::CompilerOptions, NativeCheckError> {
        Ok(tsrs_core::CompilerOptions {
            ets: Arc::new(self.ets_options()),
            ets_loader_path: CompilerPaths::one(&self.ets_loader_path)?,
            compile_sdk_version: Some(f64::from(self.compile_sdk_version)),
            target: tsrs_core::ScriptTarget(self.target as i32),
            module: tsrs_core::ModuleKind(self.module as i32),
            module_resolution: tsrs_core::ModuleResolutionKind(self.module_resolution as i32),
            lib: Some(self.lib.clone()),
            types: Some(self.types.clone()),
            type_roots: Some(CompilerPaths::many(&self.type_roots)?),
            paths: Some(CompilerPaths::map(&self.paths)?),
            paths_base_path: CompilerPaths::one(&self.paths_base_path)?,
            base_url: CompilerPaths::one(&self.base_url)?,
            no_emit: tsrs_core::bool_to_tristate(self.no_emit),
            checkers: Some(self.checkers as i64),
            incremental: tsrs_core::bool_to_tristate(self.incremental),
            ts_build_info_file: CompilerPaths::one(&self.ts_build_info_file)?,
            skip_paths_in_key_for_compilation_settings: tsrs_core::bool_to_tristate(
                self.skip_paths_in_key_for_compilation_settings,
            ),
            skip_base_url_in_key_for_compilation_settings: tsrs_core::bool_to_tristate(
                self.skip_base_url_in_key_for_compilation_settings,
            ),
            allow_js: tsrs_core::bool_to_tristate(self.allow_js),
            check_js: tsrs_core::bool_to_tristate(self.check_js),
            emit_node_modules_files: tsrs_core::bool_to_tristate(self.emit_node_modules_files),
            allow_synthetic_default_imports: tsrs_core::bool_to_tristate(
                self.allow_synthetic_default_imports,
            ),
            es_module_interop: tsrs_core::bool_to_tristate(self.es_module_interop),
            experimental_decorators: tsrs_core::bool_to_tristate(self.experimental_decorators),
            resolve_json_module: tsrs_core::bool_to_tristate(self.resolve_json_module),
            skip_lib_check: tsrs_core::bool_to_tristate(self.skip_lib_check),
            strict: tsrs_core::bool_to_tristate(self.strict),
            strict_null_checks: tsrs_core::bool_to_tristate(self.strict_null_checks),
            always_strict: tsrs_core::bool_to_tristate(self.always_strict),
            is_compile_js_har: tsrs_core::bool_to_tristate(self.is_compile_js_har),
            module_root_path: CompilerPaths::one(&self.module_root_path)?,
            max_flow_depth: self.max_flow_depth,
            ets_annotations_enable: tsrs_core::bool_to_tristate(self.ets_annotations_enable),
            need_do_ark_ts_linter: tsrs_core::bool_to_tristate(self.need_do_ark_ts_linter),
            is_compatible_version: tsrs_core::bool_to_tristate(self.is_compatible_version),
            ts_import_sendable_enable: tsrs_core::bool_to_tristate(self.ts_import_sendable_enable),
            ts_import_so_check: tsrs_core::bool_to_tristate(self.ts_import_so_check),
            package_manager_type: self.package_manager_type.clone(),
            skip_tsc_oh_module_check: tsrs_core::bool_to_tristate(self.skip_tsc_oh_module_check),
            skip_ark_ts_static_blocks_check: tsrs_core::bool_to_tristate(
                self.skip_ark_ts_static_blocks_check,
            ),
            compatible_sdk_version: self.compatible_sdk_version,
            compatible_sdk_version_stage: self.compatible_sdk_version_stage.clone(),
            skip_oh_modules_lint: tsrs_core::bool_to_tristate(self.skip_oh_modules_lint),
            enable_strict_check_oh_module: tsrs_core::bool_to_tristate(
                self.enable_strict_check_oh_module,
            ),
            disable_strict_check_paths: Some(CompilerPaths::many(
                &self.disable_strict_check_paths,
            )?),
            disable_sendable_check_rules: Some(self.disable_sendable_check_rules.clone()),
            mix_compile: tsrs_core::bool_to_tristate(self.mix_compile),
            strict_checker_only: tsrs_core::bool_to_tristate(self.strict_checker_only),
            oh_sdk_configs: Some(
                self.oh_sdk_configs
                    .iter()
                    .map(|s| {
                        Ok(tsrs_core::OhSdkConfig {
                            api_paths: CompilerPaths::many(&s.api_paths)?,
                        })
                    })
                    .collect::<Result<_, NativeCheckError>>()?,
            ),
            oh_system_modules: Some(self.oh_system_modules.clone()),
            oh_sdk_config_prefixes: Some(self.oh_sdk_config_prefixes.clone()),
            oh_fallback_module_roots: Some(CompilerPaths::many(&self.oh_fallback_module_roots)?),
            oh_loader_module_root: CompilerPaths::one(&self.oh_loader_module_root)?,
            oh_project_path: CompilerPaths::one(&self.oh_project_path)?,
            oh_external_api_paths: Some(CompilerPaths::many(&self.oh_external_api_paths)?),
            oh_package_exports: Some(CompilerPaths::map(&self.oh_package_exports)?),
            oh_runtime_os: self.oh_runtime_os.clone(),
            oh_origin_compatible_sdk_version: self.oh_origin_compatible_sdk_version.clone(),
            oh_project_root_path: CompilerPaths::one(&self.oh_project_root_path)?,
            oh_module_path: CompilerPaths::one(&self.oh_module_path)?,
            oh_all_module_paths: Some(CompilerPaths::many(&self.oh_all_module_paths)?),
            oh_global_module_paths: Some(CompilerPaths::many(&self.oh_global_module_paths)?),
            oh_ark_ui_declaration_dirs: Some(CompilerPaths::many(
                &self.oh_ark_ui_declaration_dirs,
            )?),
            oh_request_permissions: Some(self.oh_request_permissions.clone()),
            oh_syscap_intersection: Some(self.oh_syscap_intersection.clone()),
            oh_syscap_union: Some(self.oh_syscap_union.clone()),
            oh_device_types: Some(self.oh_device_types.clone()),
            oh_card_entry_files: Some(CompilerPaths::many(&self.oh_card_entry_files)?),
            oh_crossplatform: tsrs_core::bool_to_tristate(self.oh_crossplatform),
            oh_ignore_crossplatform_check: tsrs_core::bool_to_tristate(
                self.oh_ignore_crossplatform_check,
            ),
            oh_compile_mode: self.oh_compile_mode.clone(),
            oh_bundle_type: self.oh_bundle_type.clone(),
            oh_api_compatibility_check: self.oh_api_compatibility_check.clone(),
            oh_sdk_check_plugins: Some(
                self.oh_sdk_check_plugins
                    .iter()
                    .map(|p| {
                        Ok(tsrs_core::OhSdkCheckPlugin {
                            os_name: p.os_name.clone(),
                            tag: p.tag.clone(),
                            kind: p.plugin_type.clone(),
                            path: CompilerPaths::one(&p.path)?,
                            function_name: p.function_name.clone(),
                        })
                    })
                    .collect::<Result<_, NativeCheckError>>()?,
            ),
            oh_sdk_class_check_plugins: Some(
                self.oh_sdk_class_check_plugins
                    .iter()
                    .map(|p| {
                        Ok(tsrs_core::OhSdkClassCheckPlugin {
                            tag_name: p.tag_name.clone(),
                            path: CompilerPaths::one(&p.path)?,
                            class_name: p.class_name.clone(),
                        })
                    })
                    .collect::<Result<_, NativeCheckError>>()?,
            ),
            oh_sdk_plugin_project_config: self.oh_sdk_plugin_project_config.clone(),
            oh_sdk_plugin_executor: executor,
            ..Default::default()
        })
    }
    fn ets_options(&self) -> tsrs_core::EtsOptions {
        use tsrs_core::*;
        let s = &self.ets;
        let strings = |v: &Option<Vec<Box<str>>>| {
            v.as_ref()
                .map(|v| v.iter().map(ToString::to_string).collect())
        };
        let component =
            |c: &arkdown_ohos_schema::ets_options::EtsComponent| EtsComponentDeclaration {
                name: c.name.to_string(),
                r#type: c.type_name.to_string(),
                instance: c.instance.to_string(),
            };
        EtsOptions {
            render: EtsRenderOptions {
                method: strings(&s.render.method),
                decorator: strings(&s.render.decorator),
            },
            components: strings(&s.components),
            libs: strings(&s.libs),
            extend: EtsExtendOptions {
                decorator: strings(&s.extend.decorator),
                components: s
                    .extend
                    .components
                    .as_ref()
                    .map(|v| v.iter().map(component).collect()),
            },
            styles: EtsStylesOptions {
                decorator: s.styles.decorator.as_ref().map(ToString::to_string),
                component: s.styles.component.as_ref().map(component),
                property: s.styles.property.as_ref().map(ToString::to_string),
            },
            concurrent: EtsConcurrentOptions {
                decorator: s.concurrent.decorator.as_ref().map(ToString::to_string),
            },
            custom_component: s.custom_component.as_ref().map(ToString::to_string),
            property_decorators: s.property_decorators.as_ref().map(|v| {
                v.iter()
                    .map(|p| EtsPropertyDecorator {
                        name: p.name.to_string(),
                        need_initialization: p.need_initialization,
                    })
                    .collect()
            }),
            emit_decorators: s.emit_decorators.as_ref().map(|v| {
                v.iter()
                    .map(|p| EtsEmitDecorator {
                        name: p.name.to_string(),
                        emit_parameters: p.emit_parameters,
                    })
                    .collect()
            }),
            syntax_components: EtsSyntaxComponents {
                params_ui_callback: strings(&s.syntax_components.params_ui_callback),
                attr_ui_callback: s.syntax_components.attr_ui_callback.as_ref().map(|v| {
                    v.iter()
                        .map(|a| EtsAttributeCallback {
                            name: a.name.to_string(),
                            attributes: strings(&a.attributes),
                        })
                        .collect()
                }),
            },
        }
    }
}
