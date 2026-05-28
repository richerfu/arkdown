use arkdown_core::AbilityEntry;

/// An ability exported from a HAR dependency that the consuming module
/// may reference via cross-module navigation.
#[derive(Debug, Clone)]
pub struct HarAbility {
    /// Name of the HAR package (from `oh-package.json5`).
    pub har_name: String,
    /// The ability entry exported by the HAR.
    pub ability: AbilityEntry,
}

/// Collect all exported abilities from a list of HAR dependency entries.
///
/// In the real pipeline this would be driven by iterating the resolved HAR
/// packages' processed `module.json` files. For now it accepts pre-built
/// [`HarAbility`] records and returns them as-is so callers can merge them
/// into the consuming module's manifest.
pub fn collect_har_abilities(har_abilities: Vec<HarAbility>) -> Vec<HarAbility> {
    har_abilities
        .into_iter()
        .filter(|h| h.ability.exported)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_har_abilities_filters_unexported() {
        let abilities = vec![
            HarAbility {
                har_name: "myhar".to_string(),
                ability: AbilityEntry {
                    name: "ExportedAbility".to_string(),
                    src_entry: "./ets/ExportedAbility.ets".to_string(),
                    exported: true,
                    skills: vec![],
                    icon: None,
                    label: None,
                    description: None,
                    start_window_icon: None,
                    start_window_background: None,
                },
            },
            HarAbility {
                har_name: "myhar".to_string(),
                ability: AbilityEntry {
                    name: "InternalAbility".to_string(),
                    src_entry: "./ets/InternalAbility.ets".to_string(),
                    exported: false,
                    skills: vec![],
                    icon: None,
                    label: None,
                    description: None,
                    start_window_icon: None,
                    start_window_background: None,
                },
            },
        ];

        let result = collect_har_abilities(abilities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ability.name, "ExportedAbility");
    }

    #[test]
    fn test_collect_har_abilities_empty() {
        let result = collect_har_abilities(vec![]);
        assert!(result.is_empty());
    }
}
