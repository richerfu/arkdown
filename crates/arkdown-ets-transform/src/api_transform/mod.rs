pub mod event_emitter;
mod version_table;

#[cfg(test)]
use std::borrow::Cow;

use oxc_allocator::{Allocator, Vec as OxcVec};
use oxc_ast::ast::{ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind, Statement};
use oxc_ast::{AstBuilder, NONE};
#[cfg(test)]
use oxc_codegen::Codegen;
#[cfg(test)]
use oxc_parser::Parser;
use oxc_semantic::Scoping;
#[cfg(test)]
use oxc_semantic::SemanticBuilder;
#[cfg(test)]
use oxc_span::SourceType;
use oxc_span::SPAN;

use crate::kit_map::KitEntry;
#[cfg(test)]
use crate::transform::source_emit::CodegenSourceText;
use crate::ModuleExportNameText;

/// Rewrite `@kit.XxxKit` named imports to individual system module imports.
///
/// hvigor's ESMODULE pipeline keeps system API import requests in dot form and
/// records them separately for es2abc. Colon-form OHM URLs belong to preload and
/// CommonJS NAPI rewrites, not this ESMODULE import rewrite.
///
/// # Examples
///
/// ```text
/// // Input
/// import { UIAbility, Want } from "@kit.AbilityKit";
/// import { hilog } from "@kit.PerformanceAnalysisKit";
///
/// // Output
/// import UIAbility from "@ohos.app.ability.UIAbility";
/// import Want from "@ohos.app.ability.Want";
/// import hilog from "@ohos.hilog";
/// ```
///
/// Aliased imports are also handled:
/// ```text
/// import { UIAbility as UA } from "@kit.AbilityKit"
/// // -> import UA from "@ohos.app.ability.UIAbility";
/// ```
///
/// If a kit specifier is unknown or none of its named imports have a mapping the
/// original declaration is left unchanged.
pub struct KitImportRewriter;

impl KitImportRewriter {
    pub fn new(_source: &str) -> Self {
        Self
    }

    pub(crate) fn rewrite_program<'ast>(
        &self,
        allocator: &'ast Allocator,
        program: &mut oxc_ast::ast::Program<'ast>,
        scoping: &Scoping,
    ) -> bool {
        let replacement_plans = program
            .body
            .iter()
            .map(|statement| match statement {
                Statement::ImportDeclaration(import) => {
                    KitImportDeclarationRewriter::from_import(import, scoping).bindings()
                }
                Statement::LazyImportDeclaration(import) => {
                    KitImportDeclarationRewriter::from_lazy_import(import, scoping).bindings()
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let ast = AstBuilder::new(allocator);
        let mut changed = false;
        let mut body = OxcVec::new_in(&allocator);
        let old_body = std::mem::replace(&mut program.body, OxcVec::new_in(&allocator));
        for (statement, replacement_plan) in old_body.into_iter().zip(replacement_plans) {
            if let Some(imports) = replacement_plan {
                changed = true;
                for statement in KitImportEmitter::new(imports).statements(ast) {
                    body.push(statement);
                }
            } else {
                body.push(statement);
            }
        }
        program.body = body;
        changed
    }
}

struct KitImportDeclarationRewriter<'a> {
    source: &'a oxc_ast::ast::StringLiteral<'a>,
    specifiers: Option<&'a OxcVec<'a, ImportDeclarationSpecifier<'a>>>,
    import_kind: ImportOrExportKind,
    lazy: bool,
    scoping: &'a Scoping,
}

impl<'a> KitImportDeclarationRewriter<'a> {
    fn from_import(import: &'a ImportDeclaration<'a>, scoping: &'a Scoping) -> Self {
        Self {
            source: &import.source,
            specifiers: import.specifiers.as_ref(),
            import_kind: import.import_kind,
            lazy: false,
            scoping,
        }
    }

    fn from_lazy_import(
        import: &'a oxc_ast::ast::LazyImportDeclaration<'a>,
        scoping: &'a Scoping,
    ) -> Self {
        Self {
            source: &import.source,
            specifiers: import.specifiers.as_ref(),
            import_kind: ImportOrExportKind::Value,
            lazy: true,
            scoping,
        }
    }

    fn bindings(&self) -> Option<Vec<KitImportBinding>> {
        let kit_specifier = self.source.value.as_str();
        if !crate::kit_map::is_kit_specifier(kit_specifier) {
            return None;
        }

        let specifiers = self.specifiers?;
        let mut imports = Vec::new();

        for specifier in specifiers {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                continue;
            };

            let imported_name = ModuleExportNameText::new(&specifier.imported).binding_name();
            let local_name = specifier.local.name.as_str();
            let Some(kit_entry) = crate::kit_map::resolve_kit_import(kit_specifier, &imported_name)
            else {
                continue;
            };

            let import_kind = if self.import_kind.is_type()
                || specifier.import_kind.is_type()
                || specifier.local.symbol_id.get().is_some_and(|symbol_id| {
                    !self
                        .scoping
                        .get_resolved_references(symbol_id)
                        .any(|reference| reference.flags().is_value())
                }) {
                ImportOrExportKind::Type
            } else {
                ImportOrExportKind::Value
            };
            imports.push(KitImportBinding {
                local_name: local_name.to_owned(),
                module: HvigorEsmoduleModuleSpecifier::new(kit_entry.module()).to_import_request(),
                import_kind,
                lazy: self.lazy,
                binding: kit_entry,
            });
        }

        (!imports.is_empty()).then_some(imports)
    }
}

struct HvigorEsmoduleModuleSpecifier<'a> {
    module: &'a str,
}

impl<'a> HvigorEsmoduleModuleSpecifier<'a> {
    fn new(module: &'a str) -> Self {
        Self { module }
    }

    fn to_import_request(&self) -> String {
        let Some((prefix, module)) = self.module.split_once(':') else {
            return self.module.to_owned();
        };
        let Some(system_prefix) = prefix.strip_prefix('@') else {
            return self.module.to_owned();
        };
        if matches!(system_prefix, "ohos" | "hms" | "system" | "native") && !module.is_empty() {
            return [prefix, ".", module].concat();
        }
        self.module.to_owned()
    }
}

struct KitImportBinding {
    local_name: String,
    module: String,
    import_kind: ImportOrExportKind,
    lazy: bool,
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

    fn statements<'a>(&self, ast: AstBuilder<'a>) -> OxcVec<'a, Statement<'a>> {
        ast.vec_from_iter(
            self.imports
                .iter()
                .map(|import| self.import_statement(ast, import)),
        )
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
                ImportDeclarationSpecifier::ImportSpecifier(ast.alloc_import_specifier(
                    SPAN,
                    imported,
                    ast.binding_identifier(SPAN, local_name),
                    ImportOrExportKind::Value,
                ))
            }
        };
        if import.lazy {
            Statement::from(ast.module_declaration_lazy_import_declaration(
                SPAN,
                Some(ast.vec1(specifier)),
                ast.string_literal(SPAN, module, None),
                NONE,
            ))
        } else {
            Statement::from(ast.module_declaration_import_declaration(
                SPAN,
                Some(ast.vec1(specifier)),
                ast.string_literal(SPAN, module, None),
                None,
                NONE,
                import.import_kind,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewrite_source(source: &str) -> Cow<'_, str> {
        let allocator = Allocator::default();
        let source_type = SourceType::from_path("arkdown-input.ets")
            .unwrap_or_else(|_| SourceType::ts().with_module(true));
        let mut parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }
        let semantic_ret = SemanticBuilder::new().build(&parsed.program);
        if !semantic_ret.diagnostics.is_empty() {
            return Cow::Borrowed(source);
        }
        let scoping = semantic_ret.semantic.into_scoping();
        let changed = KitImportRewriter::new(source).rewrite_program(
            &allocator,
            &mut parsed.program,
            &scoping,
        );
        if changed {
            Cow::Owned(
                CodegenSourceText::new(Codegen::new().build(&parsed.program).code)
                    .without_trailing_newline(),
            )
        } else {
            Cow::Borrowed(source)
        }
    }

    #[test]
    fn test_rewrite_single_import() {
        let src = r#"import { UIAbility } from "@kit.AbilityKit";
const Ability = UIAbility;"#;
        let out = rewrite_source(src);
        assert!(
            out.contains("import UIAbility from \"@ohos.app.ability.UIAbility\""),
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
        let out = rewrite_source(src);
        assert!(
            out.contains("import UIAbility from \"@ohos.app.ability.UIAbility\""),
            "got: {out}"
        );
        assert!(
            out.contains("import Want from \"@ohos.app.ability.Want\""),
            "got: {out}"
        );
    }

    #[test]
    fn test_rewrite_lazy_kit_import_preserves_lazy_modifier() {
        let src = r#"import lazy { textToSpeech } from "@kit.CoreSpeechKit";
textToSpeech.createEngine({});"#;
        let out = rewrite_source(src);
        assert!(
            out.contains("import lazy textToSpeech from \"@hms.ai.textToSpeech\""),
            "hvigor processKitImport keeps the explicit lazy modifier: {out}"
        );
        assert!(!out.contains("@kit.CoreSpeechKit"), "got: {out}");
    }

    #[test]
    fn test_rewrite_aliased_import() {
        let src = r#"import { UIAbility as UA } from "@kit.AbilityKit";
const Ability = UA;"#;
        let out = rewrite_source(src);
        assert!(
            out.contains("import UA from \"@ohos.app.ability.UIAbility\""),
            "alias must be the binding; got: {out}"
        );
    }

    #[test]
    fn test_rewrite_hilog() {
        let src = r#"import { hilog } from "@kit.PerformanceAnalysisKit";
hilog.info(0, "tag", "message");"#;
        let out = rewrite_source(src);
        assert!(
            out.contains("import hilog from \"@ohos.hilog\""),
            "got: {out}"
        );
    }

    #[test]
    fn test_unknown_kit_kept_verbatim() {
        let src = r#"import { something } from "@kit.UnknownKit";"#;
        let out = rewrite_source(src);
        assert_eq!(out, src, "unknown kit import must be left unchanged");
    }

    #[test]
    fn test_non_kit_import_untouched() {
        let src = r#"import hilog from "@ohos:hilog";"#;
        let out = rewrite_source(src);
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
        let out = rewrite_source(src);
        assert!(
            out.contains("import { Callee } from \"@ohos.app.ability.UIAbility\""),
            "named import without alias; got: {out}"
        );
        assert!(
            !out.contains("@kit.AbilityKit"),
            "kit specifier must be gone; got: {out}"
        );
    }

    #[test]
    fn test_named_binding_with_alias() {
        // Callee is KitEntry::Named(@ohos:app.ability.UIAbility, "Callee")
        // local_name (MyCallee) != binding_name (Callee) → emit `import { Callee as MyCallee }`
        // Use the symbol as a value so the semantic analysis classifies it as a value import.
        let src = r#"import { Callee as MyCallee } from "@kit.AbilityKit";
const c = new MyCallee();"#;
        let out = rewrite_source(src);
        assert!(
            out.contains("import { Callee as MyCallee } from \"@ohos.app.ability.UIAbility\""),
            "named import with alias; got: {out}"
        );
        assert!(
            !out.contains("@kit.AbilityKit"),
            "kit specifier must be gone; got: {out}"
        );
    }

    #[test]
    fn test_single_quoted_kit_import() {
        let src = "import { hilog } from '@kit.PerformanceAnalysisKit';\nhilog.info(0, 'tag', 'message');";
        let out = rewrite_source(src);
        assert!(
            out.contains("import hilog from \"@ohos.hilog\""),
            "got: {out}"
        );
    }

    #[test]
    fn test_type_only_kit_import_uses_oxc_semantic() {
        let src = r#"import { AbilityConstant } from "@kit.AbilityKit";
type LaunchParam = AbilityConstant.StartOptions;
const text = "AbilityConstant";"#;
        let out = rewrite_source(src);
        assert!(
            out.contains("import type AbilityConstant from \"@ohos.app.ability.AbilityConstant\""),
            "type-only kit import must be decided from semantic references; got: {out}"
        );
    }
}
