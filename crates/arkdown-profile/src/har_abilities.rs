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

/// Collects exported abilities from HAR dependencies.
///
/// Mirrors hvigor's `CollectHarAbilities` task: filters the raw ability list
/// from a HAR's processed `module.json` down to only those entries that carry
/// `exported: true`, so the consuming module's manifest only references
/// abilities that are actually reachable via cross-module navigation.
pub struct HarAbilityCollector;

impl HarAbilityCollector {
    /// Create a new collector.
    pub fn new() -> Self {
        HarAbilityCollector
    }

    /// Filter `abilities` to those that are exported.
    pub fn collect_har_abilities(&self, abilities: Vec<HarAbility>) -> Vec<HarAbility> {
        abilities
            .into_iter()
            .filter(|h| h.ability.exported)
            .collect()
    }
}

impl Default for HarAbilityCollector {
    fn default() -> Self {
        HarAbilityCollector::new()
    }
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

        let result = HarAbilityCollector::new().collect_har_abilities(abilities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ability.name, "ExportedAbility");
    }

    #[test]
    fn test_collect_har_abilities_empty() {
        let result = HarAbilityCollector::new().collect_har_abilities(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_har_ability_collector_filters_unexported() {
        let collector = HarAbilityCollector::new();
        let abilities = vec![
            HarAbility {
                har_name: "pkg".to_string(),
                ability: AbilityEntry {
                    name: "Exported".to_string(),
                    src_entry: "./ets/Exported.ets".to_string(),
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
                har_name: "pkg".to_string(),
                ability: AbilityEntry {
                    name: "Internal".to_string(),
                    src_entry: "./ets/Internal.ets".to_string(),
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
        let result = collector.collect_har_abilities(abilities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ability.name, "Exported");
    }

    #[test]
    fn test_har_ability_collector_empty_input() {
        let collector = HarAbilityCollector;
        assert!(collector.collect_har_abilities(vec![]).is_empty());
    }
}
