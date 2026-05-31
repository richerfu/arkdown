mod version_table;

pub use version_table::rewrite_imports_for_version;

use oxc_allocator::Allocator;
use oxc_ast::ast::{ImportDeclaration, ImportDeclarationSpecifier, Statement};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::SourceType;

use crate::{apply_source_patches, module_export_name_text, SourcePatch};

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
    let allocator = Allocator::default();
    let source_type = SourceType::from_path("arkdown-input.ets")
        .unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return source.to_string();
    }

    let semantic_ret = SemanticBuilder::new().build(&parsed.program);
    if !semantic_ret.errors.is_empty() {
        return source.to_string();
    }
    let scoping = semantic_ret.semantic.scoping();

    let mut patches = Vec::new();
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(replacement) = rewrite_kit_import_declaration(import, scoping) else {
            continue;
        };
        patches.push(SourcePatch {
            start: import.span.start as usize,
            end: import.span.end as usize,
            replacement,
        });
    }

    apply_source_patches(source, patches)
}

fn rewrite_kit_import_declaration(
    import: &ImportDeclaration<'_>,
    scoping: &Scoping,
) -> Option<String> {
    let kit_specifier = import.source.value.as_str();
    if !crate::kit_map::is_kit_specifier(kit_specifier) {
        return None;
    }

    let specifiers = import.specifiers.as_ref()?;
    let mut out = String::new();
    let mut any_mapped = false;

    for specifier in specifiers {
        let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
            continue;
        };

        let imported_name = module_export_name_text(&specifier.imported);
        let local_name = specifier.local.name.as_str();
        let Some(ohos_module) = crate::kit_map::resolve_kit_import(kit_specifier, &imported_name)
        else {
            continue;
        };

        let type_prefix = if import.import_kind.is_type()
            || specifier.import_kind.is_type()
            || specifier.local.symbol_id.get().is_some_and(|symbol_id| {
                !scoping
                    .get_resolved_references(symbol_id)
                    .any(|reference| reference.flags().is_value())
            }) {
            "type "
        } else {
            ""
        };
        out.push_str(&format!(
            "import {type_prefix}{local_name} from \"{ohos_module}\";\n",
        ));
        any_mapped = true;
    }

    any_mapped.then(|| out.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_single_import() {
        let src = r#"import { UIAbility } from "@kit.AbilityKit";
const Ability = UIAbility;"#;
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
        let src = r#"import { UIAbility, Want } from "@kit.AbilityKit";
const Ability = UIAbility;
const value = new Want();"#;
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
        let src = r#"import { UIAbility as UA } from "@kit.AbilityKit";
const Ability = UA;"#;
        let out = rewrite_kit_imports(src);
        assert!(
            out.contains("import UA from \"@ohos:app.ability.UIAbility\""),
            "alias must be the binding; got: {out}"
        );
    }

    #[test]
    fn test_rewrite_hilog() {
        let src = r#"import { hilog } from "@kit.PerformanceAnalysisKit";
hilog.info(0, "tag", "message");"#;
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
        let src = "import { hilog } from '@kit.PerformanceAnalysisKit';\nhilog.info(0, 'tag', 'message');";
        let out = rewrite_kit_imports(src);
        assert!(
            out.contains("import hilog from \"@ohos:hilog\""),
            "got: {out}"
        );
    }

    #[test]
    fn test_type_only_kit_import_uses_oxc_semantic() {
        let src = r#"import { AbilityConstant } from "@kit.AbilityKit";
type LaunchParam = AbilityConstant.StartOptions;
const text = "AbilityConstant";"#;
        let out = rewrite_kit_imports(src);
        assert!(
            out.contains("import type AbilityConstant from \"@ohos:app.ability.AbilityConstant\""),
            "type-only kit import must be decided from semantic references; got: {out}"
        );
    }
}
