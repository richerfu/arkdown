mod version_table;

pub use version_table::rewrite_imports_for_version;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind, Program, Statement,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::{SourceType, SPAN};

use crate::kit_map::KitEntry;
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
    let mut imports = Vec::new();

    for specifier in specifiers {
        let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
            continue;
        };

        let imported_name = module_export_name_text(&specifier.imported);
        let local_name = specifier.local.name.as_str();
        let Some(kit_entry) = crate::kit_map::resolve_kit_import(kit_specifier, &imported_name)
        else {
            continue;
        };

        let import_kind = if import.import_kind.is_type()
            || specifier.import_kind.is_type()
            || specifier.local.symbol_id.get().is_some_and(|symbol_id| {
                !scoping
                    .get_resolved_references(symbol_id)
                    .any(|reference| reference.flags().is_value())
            }) {
            ImportOrExportKind::Type
        } else {
            ImportOrExportKind::Value
        };
        imports.push(KitImportBinding {
            local_name: local_name.to_owned(),
            module: kit_entry.module().to_owned(),
            import_kind,
            binding: kit_entry,
        });
    }

    (!imports.is_empty()).then(|| KitImportEmitter::new(imports).emit())
}

struct KitImportBinding {
    local_name: String,
    module: String,
    import_kind: ImportOrExportKind,
    /// Whether to emit a default or named specifier.
    binding: KitEntry,
}

struct KitImportEmitter {
    imports: Vec<KitImportBinding>,
}

impl KitImportEmitter {
    fn new(imports: Vec<KitImportBinding>) -> Self {
        Self { imports }
    }

    fn emit(&self) -> String {
        let allocator = Allocator::default();
        let ast = AstBuilder::new(&allocator);
        let statements = ast.vec_from_iter(
            self.imports
                .iter()
                .map(|import| self.import_statement(ast, import)),
        );
        let program = ast.program(
            SPAN,
            SourceType::ts().with_module(true),
            "",
            ast.vec(),
            None,
            ast.vec(),
            statements,
        );
        Self::render_program(&program)
    }

    fn import_statement<'a>(
        &self,
        ast: AstBuilder<'a>,
        import: &KitImportBinding,
    ) -> Statement<'a> {
        let local_name: &'a str = ast.allocator.alloc_str(&import.local_name);
        let module: &'a str = ast.allocator.alloc_str(&import.module);
        let specifier = match &import.binding {
            KitEntry::Default(_) => ImportDeclarationSpecifier::ImportDefaultSpecifier(
                ast.alloc_import_default_specifier(SPAN, ast.binding_identifier(SPAN, local_name)),
            ),
            KitEntry::Named(_, binding_name) => {
                let binding_name: &'a str = ast.allocator.alloc_str(binding_name);
                let is_renamed = local_name != binding_name;
                // When not renamed the OXC codegen omits the `as` clause automatically
                // because `imported` == `local`. When renamed we set `imported` to the
                // binding name so codegen emits `{ binding_name as local_name }`.
                let imported = if is_renamed {
                    ast.module_export_name_identifier_name(SPAN, binding_name)
                } else {
                    ast.module_export_name_identifier_name(SPAN, local_name)
                };
                ImportDeclarationSpecifier::ImportSpecifier(
                    ast.alloc_import_specifier(
                        SPAN,
                        imported,
                        ast.binding_identifier(SPAN, local_name),
                        ImportOrExportKind::Value,
                    ),
                )
            }
        };
        Statement::from(ast.module_declaration_import_declaration(
            SPAN,
            Some(ast.vec1(specifier)),
            ast.string_literal(SPAN, module, None),
            None,
            NONE,
            import.import_kind,
        ))
    }

    fn render_program(program: &Program<'_>) -> String {
        Codegen::new()
            .build(program)
            .code
            .trim_end_matches('\n')
            .to_owned()
    }
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
    fn test_named_binding_no_alias() {
        // Callee is KitEntry::Named(@ohos:app.ability.UIAbility, "Callee")
        // local_name == binding_name → emit `import { Callee } from "…"`
        // Use the symbol as a value (function call) so the semantic analysis
        // classifies it as a value import, not a type import.
        let src = r#"import { Callee } from "@kit.AbilityKit";
const c = new Callee();"#;
        let out = rewrite_kit_imports(src);
        assert!(
            out.contains("import { Callee } from \"@ohos:app.ability.UIAbility\""),
            "named import without alias; got: {out}"
        );
        assert!(!out.contains("@kit.AbilityKit"), "kit specifier must be gone; got: {out}");
    }

    #[test]
    fn test_named_binding_with_alias() {
        // Callee is KitEntry::Named(@ohos:app.ability.UIAbility, "Callee")
        // local_name (MyCallee) != binding_name (Callee) → emit `import { Callee as MyCallee }`
        // Use the symbol as a value so the semantic analysis classifies it as a value import.
        let src = r#"import { Callee as MyCallee } from "@kit.AbilityKit";
const c = new MyCallee();"#;
        let out = rewrite_kit_imports(src);
        assert!(
            out.contains("import { Callee as MyCallee } from \"@ohos:app.ability.UIAbility\""),
            "named import with alias; got: {out}"
        );
        assert!(!out.contains("@kit.AbilityKit"), "kit specifier must be gone; got: {out}");
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
