use arkdown_core::{
    AbilityEntry, CompileMode, ExtensionAbilityEntry, HapManifest, ModuleJson5, OhosModuleType,
};

/// Transform a raw [`ModuleJson5`] into a processed [`HapManifest`].
///
/// This mirrors the `ProcessProfile` task in hvigor:
/// - Sets `virtualMachine` from `compileApiVersion`
/// - Fills `compileMode` from build configuration
/// - Maps ability entries
pub fn process_profile(
    module_json5: &ModuleJson5,
    compile_api_version: u32,
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
        })
        .collect();

    HapManifest {
        module_name: module_json5.module.name.clone(),
        module_type: map_module_type(&module_json5.module.type_),
        device_types: module_json5.module.device_types.clone(),
        // e.g. api version 12 → "ark12.0.1.0", api version 17 → "ark17.0.1.0"
        virtual_machine: format!("ark{}.0.1.0", compile_api_version),
        compile_mode,
        abilities,
        extension_abilities,
        dependencies: vec![],
        router_map: None,
        pages: module_json5.module.pages.clone(),
        main_element: module_json5.module.main_element.clone(),
    }
}

fn map_module_type(t: &str) -> OhosModuleType {
    match t {
        "feature" => OhosModuleType::Feature,
        "shared" => OhosModuleType::Shared,
        "har" => OhosModuleType::Har,
        _ => OhosModuleType::Entry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{
        AbilityConfig, ModuleJson5Module,
    };

    fn make_module_json5(name: &str, type_: &str) -> ModuleJson5 {
        ModuleJson5 {
            module: ModuleJson5Module {
                name: name.to_string(),
                type_: type_.to_string(),
                main_element: None,
                device_types: vec!["phone".to_string()],
                pages: Some("$profile:main_pages".to_string()),
                abilities: vec![AbilityConfig {
                    name: "EntryAbility".to_string(),
                    src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                    exported: true,
                    skills: vec![],
                }],
                extension_abilities: vec![],
                src_entry: None,
                metadata: vec![],
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

    #[test]
    fn test_process_profile_api_version_35() {
        // API version 35 → "ark35.0.1.0"
        let mj = make_module_json5("feature1", "feature");
        let manifest = process_profile(&mj, 35, "com.example.app", CompileMode::EsModule);
        assert_eq!(manifest.virtual_machine, "ark35.0.1.0");
    }
}
