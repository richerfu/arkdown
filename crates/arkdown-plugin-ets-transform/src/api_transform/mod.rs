mod version_table;

pub use version_table::rewrite_imports_for_version;

/// Replace ArkUI `build()` method DSL bodies with valid JavaScript stubs.
///
/// The ArkUI declarative syntax like `Row() { Column() { } }.width()` is not
/// valid TypeScript and cannot be parsed by standard parsers. For bundling
/// purposes we replace the entire body with `return;` so the class structure
/// remains valid TypeScript.
///
/// Only exact `build()` method declarations are matched — names such as
/// `rebuild()` or `_build()` are left untouched.
pub fn stub_arkui_build_bodies(source: &str) -> String {
    let mut result = source.to_string();
    let mut search_from = 0usize;

    loop {
        // Look for the literal "build()" starting from our current position.
        let Some(rel_pos) = result[search_from..].find("build()") else {
            break;
        };
        let abs_pos = search_from + rel_pos;

        // Reject occurrences that are part of a longer identifier, e.g. "rebuild()".
        let pre_char = result[..abs_pos].chars().last();
        if matches!(pre_char, Some(c) if c.is_alphanumeric() || c == '_') {
            search_from = abs_pos + 1;
            continue;
        }

        // Find the opening brace '{' that starts the method body.
        // There may be optional whitespace between "build()" and "{".
        let after_decl = abs_pos + "build()".len();
        let Some(rel_brace) = result[after_decl..].find('{') else {
            break;
        };
        // Make sure there is only whitespace between "build()" and "{".
        let between = &result[after_decl..after_decl + rel_brace];
        if !between.chars().all(|c| c.is_whitespace()) {
            search_from = abs_pos + 1;
            continue;
        }

        let abs_brace = after_decl + rel_brace;

        // Walk forward from the opening brace counting depth to find the
        // matching closing brace.
        let mut depth = 0usize;
        let mut end_pos: Option<usize> = None;
        for (i, ch) in result[abs_brace..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_pos = Some(abs_brace + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end_pos else {
            // Unbalanced braces — leave the rest of the source unchanged.
            break;
        };

        // Replace the entire "build() { ... }" span with the stub.
        let stub = "build() { return; }";
        result = format!("{}{}{}", &result[..abs_pos], stub, &result[end + 1..]);
        // Advance past the stub so we do not re-scan it.
        search_from = abs_pos + stub.len();
    }

    result
}

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
        Regex::new(r#"import\s*\{([^}]+)\}\s*from\s*["'](@kit\.[^"']+)["']\s*;?"#)
            .expect("kit import regex must compile")
    });

    let mut result = String::with_capacity(source.len());
    let mut last_end = 0;
    let source_without_imports = kit_re.replace_all(source, "");

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

            if let Some(ohos_module) = crate::kit_map::resolve_kit_import(kit_spec, original_name) {
                let type_prefix = if binding_has_value_usage(&source_without_imports, binding) {
                    ""
                } else {
                    "type "
                };
                fragment.push_str(&format!(
                    "import {type_prefix}{binding} from \"{ohos_module}\";\n",
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

fn binding_has_value_usage(source: &str, binding: &str) -> bool {
    let mut seen = false;
    for (start, _) in identifier_occurrences(source, binding) {
        seen = true;
        if !is_type_position(source, start) {
            return true;
        }
    }
    !seen
}

fn identifier_occurrences<'a>(
    source: &'a str,
    binding: &'a str,
) -> impl Iterator<Item = (usize, usize)> + 'a {
    source
        .match_indices(binding)
        .filter_map(move |(start, text)| {
            let end = start + text.len();
            let before = source[..start].chars().next_back();
            let after = source[end..].chars().next();
            let before_ok = before.is_none_or(|ch| !is_ident_continue(ch));
            let after_ok = after.is_none_or(|ch| !is_ident_continue(ch));
            (before_ok && after_ok).then_some((start, end))
        })
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn is_type_position(source: &str, start: usize) -> bool {
    let prefix = &source[..start];
    let significant_start = prefix
        .rfind(['\n', ';', '{', '}'])
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let segment = &prefix[significant_start..];

    if segment.rfind(':').is_some_and(|colon| {
        !segment[colon + 1..]
            .chars()
            .any(|ch| matches!(ch, '=' | ';' | '{' | '}'))
    }) {
        return true;
    }

    let trimmed = prefix.trim_end();
    trimmed.ends_with(" as")
        || trimmed.ends_with("type")
        || trimmed.ends_with("interface")
        || trimmed.ends_with("implements")
        || trimmed.ends_with('|')
        || trimmed.ends_with('&')
        || trimmed.ends_with('<')
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
        assert!(
            !out.contains("@kit.AbilityKit"),
            "kit specifier must be gone; got: {out}"
        );
    }

    #[test]
    fn test_rewrite_multiple_imports_same_kit() {
        let src = r#"import { UIAbility, Want } from "@kit.AbilityKit";"#;
        let out = rewrite_kit_imports(src);
        assert!(
            out.contains("import UIAbility from \"@ohos:app.ability.UIAbility\""),
            "got: {out}"
        );
        assert!(
            out.contains("import Want from \"@ohos:app.ability.Want\""),
            "got: {out}"
        );
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
        assert!(
            out.contains("import hilog from \"@ohos:hilog\""),
            "got: {out}"
        );
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
        assert!(
            out.contains("import hilog from \"@ohos:hilog\""),
            "got: {out}"
        );
    }

    // -----------------------------------------------------------------------
    // stub_arkui_build_bodies
    // -----------------------------------------------------------------------

    #[test]
    fn test_stub_empty_build_body() {
        let src = "class Index extends ViewPU {\n  build() {}\n}";
        let out = super::stub_arkui_build_bodies(src);
        // An already-empty body is replaced with the canonical stub.
        assert!(out.contains("build() { return; }"), "got: {out}");
        assert!(
            !out.contains("build() build()"),
            "build() declaration must not be duplicated; got: {out}"
        );
    }

    #[test]
    fn test_stub_simple_arkui_build() {
        let src = "class Index extends ViewPU {\n  build() {\n    Row() {\n      Column() {\n        Text('hi')\n      }\n    }\n  }\n}";
        let out = super::stub_arkui_build_bodies(src);
        assert!(
            out.contains("build() { return; }"),
            "body must be stubbed; got: {out}"
        );
        assert!(
            !out.contains("build() build()"),
            "build() declaration must not be duplicated; got: {out}"
        );
        assert!(!out.contains("Row()"), "ArkUI DSL must be gone; got: {out}");
    }

    #[test]
    fn test_stub_does_not_touch_rebuild() {
        let src = "class Foo {\n  rebuild() {\n    doSomething();\n  }\n}";
        let out = super::stub_arkui_build_bodies(src);
        // "rebuild()" must not be touched.
        assert!(
            out.contains("rebuild()"),
            "rebuild() must be preserved; got: {out}"
        );
        assert!(
            out.contains("doSomething()"),
            "body must be preserved; got: {out}"
        );
    }

    #[test]
    fn test_stub_nested_braces() {
        // build() body contains an object literal — nested braces must balance correctly.
        let src = "class C extends ViewPU {\n  build() {\n    if (true) {\n      let x = { a: 1 };\n    }\n  }\n  other() { return 42; }\n}";
        let out = super::stub_arkui_build_bodies(src);
        assert!(out.contains("build() { return; }"), "got: {out}");
        assert!(
            !out.contains("build() build()"),
            "build() declaration must not be duplicated; got: {out}"
        );
        // other() must remain untouched.
        assert!(
            out.contains("other() { return 42; }"),
            "other() body must be unchanged; got: {out}"
        );
    }

    #[test]
    fn test_stub_no_build_passthrough() {
        let src = "class Foo {\n  render() {\n    return null;\n  }\n}";
        let out = super::stub_arkui_build_bodies(src);
        assert_eq!(
            out, src,
            "source without build() must be returned unchanged"
        );
    }
}
