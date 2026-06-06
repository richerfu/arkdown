use arkdown_core::{
    AbilityEntry, ArkdownError, CompileMode, ExtensionAbilityEntry, HapManifest, HspDependency,
    ModuleJson5, OhosModuleType,
};

/// Validate the raw `compileMode` field in a [`ModuleJson5`].
///
/// Hvigor supports two modes:
/// - `"esmodule"` (default) — Stage model, ECMAScript module output.  Supported.
/// - `"jsbundle"` — Legacy FA model, CommonJS bundle output.  **Not supported** by arkdown.
///
/// When a module declares `compileMode: "jsbundle"`, this function returns an
/// `ArkdownError` describing the limitation.  This matches hvigor's behavior of
/// producing JSBUNDLE output for such modules; arkdown explicitly rejects the
/// mode rather than silently producing incorrect ESModule output.
pub fn validate_compile_mode(module_json5: &ModuleJson5, file: &str) -> Result<(), ArkdownError> {
    if let Some(mode) = module_json5.module.compile_mode.as_deref() {
        if mode.eq_ignore_ascii_case("jsbundle") {
            return Err(ArkdownError::Bundler(format!(
                "UNSUPPORTED_COMPILE_MODE: module '{}' in '{}' uses compileMode \"jsbundle\" \
                 (legacy FA model).  arkdown only supports Stage model (compileMode \
                 \"esmodule\").  Migrate to the Stage model to use arkdown.",
                module_json5.module.name, file
            )));
        }
    }
    Ok(())
}

/// Transform a raw [`ModuleJson5`] into a processed [`HapManifest`].
///
/// This mirrors the `ProcessProfile` task in hvigor:
/// - Sets `virtualMachine`
/// - Fills `compileMode` from build configuration
/// - Maps ability entries
pub fn process_profile(
    module_json5: &ModuleJson5,
    compile_api_version: u32,
    _bundle_name: &str,
    compile_mode: CompileMode,
) -> HapManifest {
    let ark_vm_version = format!("{}.0.1.0", compile_api_version);
    process_profile_with_ark_vm_version(module_json5, &ark_vm_version, _bundle_name, compile_mode)
}

/// Transform a raw [`ModuleJson5`] using an Ark VM version discovered from the SDK.
///
/// hvigor obtains this from the SDK Ark tooling and writes
/// `module.virtualMachine` as `ark<version>`, for example `ark12.0.6.0`.
pub fn process_profile_with_ark_vm_version(
    module_json5: &ModuleJson5,
    ark_vm_version: &str,
    _bundle_name: &str,
    compile_mode: CompileMode,
) -> HapManifest {
    let abilities: Vec<AbilityEntry> = module_json5
        .module
        .abilities
        .iter()
        .map(|a| AbilityEntry {
            name: a.name.clone(),
            src_entry: a.src_entry.clone(),
            exported: a.exported,
            skills: a.skills.clone(),
            icon: a.icon.clone(),
            label: a.label.clone(),
            description: a.description.clone(),
            start_window_icon: a.start_window_icon.clone(),
            start_window_background: a.start_window_background.clone(),
        })
        .collect();

    let extension_abilities: Vec<ExtensionAbilityEntry> = module_json5
        .module
        .extension_abilities
        .iter()
        .map(|e| ExtensionAbilityEntry {
            name: e.name.clone(),
            src_entry: e.src_entry.clone(),
            extension_type: e.extension_type.clone(),
            label: e.label.clone(),
            description: e.description.clone(),
            metadata: e.metadata.clone(),
        })
        .collect();

    HapManifest {
        module_name: module_json5.module.name.clone(),
        module_type: OhosModuleType::from_type_str(&module_json5.module.type_),
        device_types: module_json5.module.device_types.clone(),
        virtual_machine: ark_virtual_machine_from_version(ark_vm_version),
        compile_mode,
        abilities,
        extension_abilities,
        // Populate HSP dependencies from module.json5#module.dependencies.
        // Hvigor's ProcessProfile.addModuleDependencies() collects HSP dependency
        // metadata; we use the raw field from module.json5 for each dep entry.
        dependencies: module_json5
            .module
            .dependencies
            .iter()
            .filter_map(|dep| {
                let bundle = dep.get("bundleName").and_then(|v| v.as_str())?;
                let module_n = dep.get("moduleName").and_then(|v| v.as_str())?;
                Some(HspDependency {
                    bundle_name: bundle.to_string(),
                    module_name: module_n.to_string(),
                })
            })
            .collect(),
        router_map: module_json5.module.router_map.clone(),
        pages: module_json5.module.pages.clone(),
        main_element: module_json5.module.main_element.clone(),
        description: module_json5.module.description.clone(),
        src_entry: module_json5.module.src_entry.clone(),
    }
}

/// Convert a raw SDK Ark VM version into the `module.json` field value.
pub fn ark_virtual_machine_from_version(ark_vm_version: &str) -> String {
    if ark_vm_version.starts_with("ark") {
        ark_vm_version.to_string()
    } else {
        format!("ark{}", ark_vm_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{AbilityConfig, ModuleJson5Module};

    fn make_module_json5(name: &str, type_: &str) -> ModuleJson5 {
        ModuleJson5 {
            module: ModuleJson5Module {
                name: name.to_string(),
                type_: type_.to_string(),
                main_element: None,
                device_types: vec!["phone".to_string()],
                pages: Some("$profile:main_pages".to_string()),
                router_map: None,
                description: None,
                abilities: vec![AbilityConfig {
                    name: "EntryAbility".to_string(),
                    src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                    exported: true,
                    skills: vec![],
                    icon: None,
                    label: None,
                    description: None,
                    start_window_icon: None,
                    start_window_background: None,
                }],
                extension_abilities: vec![],
                src_entry: None,
                metadata: vec![],
                app_startup: None,
                cross_app_shared_config: Default::default(),
                ..Default::default()
            },
        }
    }

    /// API version 12 → virtualMachine = "ark12.0.1.0"
    #[test]
    fn test_process_profile_sets_virtual_machine() {
        let mj = make_module_json5("entry", "entry");
        let manifest = process_profile(&mj, 12, "com.example.app", CompileMode::EsModule);
        assert_eq!(manifest.virtual_machine, "ark12.0.1.0");
        assert_eq!(manifest.module_name, "entry");
        assert_eq!(manifest.module_type, OhosModuleType::Entry);
        assert_eq!(manifest.compile_mode, CompileMode::EsModule);
        assert_eq!(manifest.abilities.len(), 1);
        assert_eq!(manifest.abilities[0].name, "EntryAbility");
        assert_eq!(
            manifest.abilities[0].src_entry,
            "./ets/entryability/EntryAbility.ets"
        );
    }

    #[test]
    fn test_process_profile_accepts_sdk_ark_vm_version() {
        let mj = make_module_json5("entry", "entry");
        let manifest = process_profile_with_ark_vm_version(
            &mj,
            "12.0.6.0",
            "com.example.app",
            CompileMode::EsModule,
        );
        assert_eq!(manifest.virtual_machine, "ark12.0.6.0");
    }

    #[test]
    fn test_ark_virtual_machine_from_version_accepts_prefixed_value() {
        assert_eq!(
            ark_virtual_machine_from_version("ark12.0.6.0"),
            "ark12.0.6.0"
        );
    }

    #[test]
    fn test_process_profile_maps_module_types() {
        let cases = [
            ("feature", OhosModuleType::Feature),
            ("shared", OhosModuleType::Shared),
            ("har", OhosModuleType::Har),
            ("entry", OhosModuleType::Entry),
            ("unknown", OhosModuleType::Entry),
        ];
        for (type_str, expected) in cases {
            let mj = make_module_json5("mod", type_str);
            let manifest = process_profile(&mj, 10, "com.example.app", CompileMode::EsModule);
            assert_eq!(manifest.module_type, expected, "type_ = {type_str}");
        }
    }

    /// JSBUNDLE compile mode must be rejected with a clear hard error.
    /// Section 3.7: "JSBUNDLE fixture 一致或拒绝一致" — arkdown rejects JSBUNDLE.
    #[test]
    fn test_validate_compile_mode_rejects_jsbundle() {
        let mut mj = make_module_json5("entry", "entry");
        mj.module.compile_mode = Some("jsbundle".to_string());
        let result = validate_compile_mode(&mj, "entry/src/main/module.json5");
        assert!(result.is_err(), "jsbundle must be rejected");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("UNSUPPORTED_COMPILE_MODE"),
            "error must contain UNSUPPORTED_COMPILE_MODE, got: {err}"
        );
        assert!(
            err.contains("jsbundle"),
            "error must mention jsbundle, got: {err}"
        );
    }

    /// JSBUNDLE reject is case-insensitive (hvigor accepts both cases).
    #[test]
    fn test_validate_compile_mode_rejects_jsbundle_case_insensitive() {
        let mut mj = make_module_json5("entry", "entry");
        mj.module.compile_mode = Some("JSBUNDLE".to_string());
        assert!(
            validate_compile_mode(&mj, "module.json5").is_err(),
            "JSBUNDLE (uppercase) must also be rejected"
        );
    }

    /// esmodule mode is valid and must not produce an error.
    #[test]
    fn test_validate_compile_mode_accepts_esmodule() {
        let mut mj = make_module_json5("entry", "entry");
        mj.module.compile_mode = Some("esmodule".to_string());
        assert!(
            validate_compile_mode(&mj, "module.json5").is_ok(),
            "esmodule must be accepted"
        );
    }

    /// Absent compileMode field (default) is valid (esmodule is the default).
    #[test]
    fn test_validate_compile_mode_accepts_absent() {
        let mj = make_module_json5("entry", "entry");
        // compile_mode is None by default in ModuleJson5Module
        assert_eq!(mj.module.compile_mode, None);
        assert!(
            validate_compile_mode(&mj, "module.json5").is_ok(),
            "absent compileMode must be accepted (defaults to esmodule)"
        );
    }

    #[test]
    fn test_process_profile_maps_hsp_dependencies() {
        let mut mj = make_module_json5("entry", "entry");
        mj.module.dependencies = vec![
            serde_json::json!({
                "bundleName": "com.example.shared",
                "moduleName": "shared_hsp"
            }),
            serde_json::json!({
                "bundleName": "com.example.incomplete"
            }),
        ];

        let manifest = process_profile(&mj, 12, "com.example.app", CompileMode::EsModule);

        assert_eq!(manifest.dependencies.len(), 1);
        assert_eq!(manifest.dependencies[0].bundle_name, "com.example.shared");
        assert_eq!(manifest.dependencies[0].module_name, "shared_hsp");
    }

    #[test]
    fn test_process_profile_api_version_35() {
        // API version 35 → "ark35.0.1.0"
        let mj = make_module_json5("feature1", "feature");
        let manifest = process_profile(&mj, 35, "com.example.app", CompileMode::EsModule);
        assert_eq!(manifest.virtual_machine, "ark35.0.1.0");
    }
}
