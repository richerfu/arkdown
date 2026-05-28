//! Maps `@kit.XxxKit` named imports to their `@ohos:xxx` source module paths.
//!
//! The ArkRuntime's system module registry only knows `@ohos:xxx` module paths.
//! hvigorw's `ets-loader` performs this rewrite before bundling; we replicate it
//! here so the bundled ABC contains resolvable module specifiers.
//!
//! Derived from the OpenHarmony SDK declaration files and confirmed against
//! the module strings observed in `modules.abc`.

/// Resolve a single named import from a `@kit.XxxKit` specifier to its
/// `@ohos:xxx` source module path.
///
/// Returns `None` if the kit specifier is unknown or the named export has no
/// known mapping (caller should fall back to keeping the original import).
pub fn resolve_kit_import(kit_specifier: &str, named_import: &str) -> Option<&'static str> {
    match (kit_specifier, named_import) {
        // ── @kit.AbilityKit ────────────────────────────────────────────────
        ("@kit.AbilityKit", "UIAbility") => Some("@ohos:app.ability.UIAbility"),
        ("@kit.AbilityKit", "AbilityConstant") => Some("@ohos:app.ability.AbilityConstant"),
        ("@kit.AbilityKit", "Want") => Some("@ohos:app.ability.Want"),
        ("@kit.AbilityKit", "ConfigurationConstant") => {
            Some("@ohos:app.ability.ConfigurationConstant")
        }
        ("@kit.AbilityKit", "AbilityLifecycleCallback") => {
            Some("@ohos:app.ability.AbilityLifecycleCallback")
        }
        ("@kit.AbilityKit", "UIExtensionAbility") => {
            Some("@ohos:app.ability.UIExtensionAbility")
        }
        ("@kit.AbilityKit", "ExtensionAbility") => Some("@ohos:app.ability.ExtensionAbility"),
        ("@kit.AbilityKit", "ServiceExtensionAbility") => {
            Some("@ohos:app.ability.ServiceExtensionAbility")
        }
        ("@kit.AbilityKit", "FormExtensionAbility") => {
            Some("@ohos:app.ability.FormExtensionAbility")
        }
        ("@kit.AbilityKit", "BackupExtensionAbility") => {
            Some("@ohos:application.BackupExtensionAbility")
        }
        ("@kit.AbilityKit", "BundleVersion") => Some("@ohos:application.BackupExtensionAbility"),
        // Catch-all for unmapped AbilityKit symbols
        ("@kit.AbilityKit", _) => Some("@ohos:app.ability.abilityManager"),

        // ── @kit.PerformanceAnalysisKit ───────────────────────────────────
        ("@kit.PerformanceAnalysisKit", "hilog") => Some("@ohos:hilog"),
        ("@kit.PerformanceAnalysisKit", "hiTraceMeter") => Some("@ohos:hiTraceMeter"),
        ("@kit.PerformanceAnalysisKit", "hiAppEvent") => Some("@ohos:hiAppEvent"),

        // ── @kit.ArkUI ────────────────────────────────────────────────────
        ("@kit.ArkUI", "window") => Some("@ohos:window"),
        ("@kit.ArkUI", "webview") => Some("@ohos:web.webview"),
        ("@kit.ArkUI", "display") => Some("@ohos:display"),
        ("@kit.ArkUI", "animator") => Some("@ohos:animator"),

        // ── @kit.ArkTS ────────────────────────────────────────────────────
        ("@kit.ArkTS", "util") => Some("@ohos:util"),
        ("@kit.ArkTS", "buffer") => Some("@ohos:buffer"),
        ("@kit.ArkTS", "collections") => Some("@ohos:collections"),

        // ── @kit.CoreFileKit ──────────────────────────────────────────────
        // All CoreFileKit exports live in @ohos:file.fs
        ("@kit.CoreFileKit", _) => Some("@ohos:file.fs"),

        // ── @kit.NetworkKit ───────────────────────────────────────────────
        ("@kit.NetworkKit", "http") => Some("@ohos:net.http"),
        ("@kit.NetworkKit", "socket") => Some("@ohos:net.socket"),
        ("@kit.NetworkKit", "connection") => Some("@ohos:net.connection"),

        // ── @kit.BasicServicesKit ─────────────────────────────────────────
        ("@kit.BasicServicesKit", "commonEventManager") => Some("@ohos:commonEventManager"),
        ("@kit.BasicServicesKit", "notificationManager") => Some("@ohos:notificationManager"),

        _ => None,
    }
}

/// Returns `true` if `spec` is a `@kit.XxxKit` module specifier.
pub fn is_kit_specifier(spec: &str) -> bool {
    spec.starts_with("@kit.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ability_kit_ui_ability() {
        assert_eq!(
            resolve_kit_import("@kit.AbilityKit", "UIAbility"),
            Some("@ohos:app.ability.UIAbility")
        );
    }

    #[test]
    fn test_ability_kit_catchall() {
        // An unmapped AbilityKit export falls back to abilityManager
        assert_eq!(
            resolve_kit_import("@kit.AbilityKit", "UnknownExport"),
            Some("@ohos:app.ability.abilityManager")
        );
    }

    #[test]
    fn test_performance_kit_hilog() {
        assert_eq!(
            resolve_kit_import("@kit.PerformanceAnalysisKit", "hilog"),
            Some("@ohos:hilog")
        );
    }

    #[test]
    fn test_core_file_kit_catchall() {
        assert_eq!(
            resolve_kit_import("@kit.CoreFileKit", "fileio"),
            Some("@ohos:file.fs")
        );
    }

    #[test]
    fn test_unknown_kit_returns_none() {
        assert_eq!(resolve_kit_import("@kit.UnknownKit", "something"), None);
    }

    #[test]
    fn test_is_kit_specifier() {
        assert!(is_kit_specifier("@kit.AbilityKit"));
        assert!(is_kit_specifier("@kit.ArkUI"));
        assert!(!is_kit_specifier("@ohos:hilog"));
        assert!(!is_kit_specifier("@ohos/hilog"));
    }
}
