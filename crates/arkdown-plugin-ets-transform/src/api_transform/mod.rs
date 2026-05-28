mod version_table;

pub use version_table::rewrite_imports_for_version;

/// Apply all API-version-gated source transforms.
///
/// Order of transforms:
/// 1. `@kit.XxxKit` → individual `@ohos:xxx` imports (always applied for API >= 9)
/// 2. `@ohos/* → @system.*` rewrite for API < 9 (FA model)
pub fn apply(source: &str, api_version: u32) -> String {
    let after_kit = rewrite_kit_imports(source);
    rewrite_imports_for_version(&after_kit, api_version)
}

/// Rewrite `@kit.XxxKit` named imports to individual `@ohos:xxx` default imports.
///
/// hvigorw's `ets-loader` performs this conversion before bundling because the
/// ArkRuntime system module registry only recognises `@ohos:xxx` module paths.
///
/// # Examples
///
/// ```text
/// // Input
/// import { UIAbility, Want } from "@kit.AbilityKit";
/// import { hilog } from "@kit.PerformanceAnalysisKit";
///
/// // Output
/// import UIAbility from "@ohos:app.ability.UIAbility";
/// import Want from "@ohos:app.ability.Want";
/// import hilog from "@ohos:hilog";
/// ```
///
/// Aliased imports are also handled:
/// ```text
/// import { UIAbility as UA } from "@kit.AbilityKit"
/// // → import UA from "@ohos:app.ability.UIAbility";
/// ```
///
/// If a kit specifier is unknown or none of its named imports have a mapping the
/// original declaration is left unchanged.
pub fn rewrite_kit_imports(source: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    // Pattern: import { A, B, C } from "@kit.XxxKit"  (single or double quotes)
    static KIT_RE: OnceLock<Regex> = OnceLock::new();
    let kit_re = KIT_RE.get_or_init(|| {
        Regex::new(r#"import\s*\{([^}]+)\}\s*from\s*["'](@kit\.[^"']+)["']"#)
            .expect("kit import regex must compile")
    });

    let mut result = String::with_capacity(source.len());
    let mut last_end = 0;

    for cap in kit_re.captures_iter(source) {
        let full_match = cap.get(0).unwrap();
        let names_str = cap[1].trim();
        let kit_spec = cap[2].trim();

        // Append everything before this match unchanged.
        result.push_str(&source[last_end..full_match.start()]);

        let mut any_mapped = false;
        let mut fragment = String::new();

        for name_part in names_str.split(',') {
            let name = name_part.trim();
            if name.is_empty() {
                continue;
            }

            // Handle "OriginalName as Alias" form.
            let (original_name, binding) = if let Some(idx) = name.find(" as ") {
                let orig = name[..idx].trim();
                let alias = name[idx + 4..].trim();
                (orig, alias)
            } else {
                (name, name)
            };

            if let Some(ohos_module) =
                crate::kit_map::resolve_kit_import(kit_spec, original_name)
            {
                fragment.push_str(&format!(
                    "import {} from \"{}\";\n",
                    binding, ohos_module
                ));
                any_mapped = true;
            }
        }

        if any_mapped {
            // Remove trailing newline so the caller sees one blank line at most.
            result.push_str(fragment.trim_end_matches('\n'));
        } else {
            // Nothing mapped — keep the original import verbatim.
            result.push_str(full_match.as_str());
        }

        last_end = full_match.end();
    }

    result.push_str(&source[last_end..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_single_import() {
        let src = r#"import { UIAbility } from "@kit.AbilityKit";"#;
        let out = rewrite_kit_imports(src);
        assert!(
            out.contains("import UIAbility from \"@ohos:app.ability.UIAbility\""),
            "got: {out}"
        );
        assert!(!out.contains("@kit.AbilityKit"), "kit specifier must be gone; got: {out}");
    }

    #[test]
    fn test_rewrite_multiple_imports_same_kit() {
        let src = r#"import { UIAbility, Want } from "@kit.AbilityKit";"#;
        let out = rewrite_kit_imports(src);
        assert!(out.contains("import UIAbility from \"@ohos:app.ability.UIAbility\""), "got: {out}");
        assert!(out.contains("import Want from \"@ohos:app.ability.Want\""), "got: {out}");
    }

    #[test]
    fn test_rewrite_aliased_import() {
        let src = r#"import { UIAbility as UA } from "@kit.AbilityKit";"#;
        let out = rewrite_kit_imports(src);
        assert!(
            out.contains("import UA from \"@ohos:app.ability.UIAbility\""),
            "alias must be the binding; got: {out}"
        );
    }

    #[test]
    fn test_rewrite_hilog() {
        let src = r#"import { hilog } from "@kit.PerformanceAnalysisKit";"#;
        let out = rewrite_kit_imports(src);
        assert!(out.contains("import hilog from \"@ohos:hilog\""), "got: {out}");
    }

    #[test]
    fn test_unknown_kit_kept_verbatim() {
        let src = r#"import { something } from "@kit.UnknownKit";"#;
        let out = rewrite_kit_imports(src);
        assert_eq!(out, src, "unknown kit import must be left unchanged");
    }

    #[test]
    fn test_non_kit_import_untouched() {
        let src = r#"import hilog from "@ohos:hilog";"#;
        let out = rewrite_kit_imports(src);
        assert_eq!(out, src);
    }

    #[test]
    fn test_single_quoted_kit_import() {
        let src = "import { hilog } from '@kit.PerformanceAnalysisKit';";
        let out = rewrite_kit_imports(src);
        assert!(out.contains("import hilog from \"@ohos:hilog\""), "got: {out}");
    }
}
