use crate::text::JsStrText;
use oxc_allocator::Vec as ArenaVec;
use oxc_ast::ast::{
    BindingIdentifier, ExportSpecifier, ImportDeclarationSpecifier, ImportOrExportKind,
    ModuleExportName, Statement, StringLiteral,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::{Span, SPAN};
use oxc_str::{Ident, Str};

#[derive(Debug)]
pub(super) struct NamedImportBinding {
    pub(super) imported: ModuleBindingName,
    pub(super) imported_span: Span,
    pub(super) local: String,
    pub(super) local_span: Span,
    pub(super) span: Span,
}

#[derive(Debug)]
pub(super) enum ValueImportBinding {
    Default {
        local: String,
        local_span: Span,
        span: Span,
    },
    Namespace {
        local: String,
        local_span: Span,
        span: Span,
    },
    Named(NamedImportBinding),
}

#[derive(Debug)]
pub(super) enum TypeImportBinding {
    Default {
        local: String,
        local_span: Span,
        span: Span,
    },
    Named(NamedImportBinding),
}

#[derive(Debug, Clone)]
pub(super) struct ModuleStringLiteral {
    value: String,
    raw: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum ModuleBindingName {
    Identifier(String),
    StringLiteral(ModuleStringLiteral),
}

#[derive(Debug)]
pub(super) struct ExportBinding {
    local: ModuleBindingName,
    exported: ModuleBindingName,
}

pub(super) struct ImportSplitEmitter {
    source: ModuleStringLiteral,
    value_bindings: Vec<ValueImportBinding>,
    type_bindings: Vec<TypeImportBinding>,
}

impl ImportSplitEmitter {
    pub(super) fn new(
        source: ModuleStringLiteral,
        value_bindings: Vec<ValueImportBinding>,
        type_bindings: Vec<TypeImportBinding>,
    ) -> Self {
        Self {
            source,
            value_bindings,
            type_bindings,
        }
    }

    pub(super) fn statements<'a>(&self, ast: &AstBuilder<'a>) -> ArenaVec<'a, Statement<'a>> {
        let mut statements = ArenaVec::new_in(ast);
        if !self.value_bindings.is_empty() {
            statements.push(self.import_statement(ast, ImportOrExportKind::Value));
        }
        for statement in self.type_import_statements(ast) {
            statements.push(statement);
        }
        statements
    }

    fn type_import_statements<'a>(&self, ast: &AstBuilder<'a>) -> ArenaVec<'a, Statement<'a>> {
        let mut statements = ArenaVec::new_in(ast);
        let default_binding = self.type_bindings.iter().find_map(|binding| match binding {
            TypeImportBinding::Default {
                local,
                local_span,
                span,
            } => Some((local.as_str(), *local_span, *span)),
            TypeImportBinding::Named(_) => None,
        });
        if let Some((local, local_span, span)) = default_binding {
            let specifiers = ArenaVec::from_value_in(
                ImportDeclarationSpecifier::new_import_default_specifier(
                    span,
                    BindingIdentifier::new(local_span, Ident::from_str_in(local, ast), ast),
                    ast,
                ),
                ast,
            );
            statements.push(Statement::new_import_declaration(
                SPAN,
                Some(specifiers),
                self.source.to_ast(ast),
                None,
                None,
                ImportOrExportKind::Type,
                ast,
            ));
        }

        let named_bindings = self
            .type_bindings
            .iter()
            .filter_map(|binding| match binding {
                TypeImportBinding::Default { .. } => None,
                TypeImportBinding::Named(binding) => Some(binding),
            });
        let specifiers = ArenaVec::from_iter_in(
            named_bindings.map(|binding| {
                ImportDeclarationSpecifier::new_import_specifier(
                    binding.span,
                    binding
                        .imported
                        .to_ast_with_span(ast, binding.imported_span),
                    BindingIdentifier::new(
                        binding.local_span,
                        Ident::from_str_in(&binding.local, ast),
                        ast,
                    ),
                    ImportOrExportKind::Value,
                    ast,
                )
            }),
            ast,
        );
        if !specifiers.is_empty() {
            statements.push(Statement::new_import_declaration(
                SPAN,
                Some(specifiers),
                self.source.to_ast(ast),
                None,
                None,
                ImportOrExportKind::Type,
                ast,
            ));
        }

        statements
    }

    fn import_statement<'a>(
        &self,
        ast: &AstBuilder<'a>,
        import_kind: ImportOrExportKind,
    ) -> Statement<'a> {
        let specifiers = if import_kind.is_type() {
            ArenaVec::from_iter_in(
                self.type_bindings.iter().map(|binding| match binding {
                    TypeImportBinding::Default {
                        local,
                        local_span,
                        span,
                    } => ImportDeclarationSpecifier::new_import_default_specifier(
                        *span,
                        BindingIdentifier::new(*local_span, Ident::from_str_in(local, ast), ast),
                        ast,
                    ),
                    TypeImportBinding::Named(binding) => {
                        ImportDeclarationSpecifier::new_import_specifier(
                            binding.span,
                            binding
                                .imported
                                .to_ast_with_span(ast, binding.imported_span),
                            BindingIdentifier::new(
                                binding.local_span,
                                Ident::from_str_in(&binding.local, ast),
                                ast,
                            ),
                            ImportOrExportKind::Value,
                            ast,
                        )
                    }
                }),
                ast,
            )
        } else {
            ArenaVec::from_iter_in(
                self.value_bindings.iter().map(|binding| match binding {
                    ValueImportBinding::Default {
                        local,
                        local_span,
                        span,
                    } => ImportDeclarationSpecifier::new_import_default_specifier(
                        *span,
                        BindingIdentifier::new(*local_span, Ident::from_str_in(local, ast), ast),
                        ast,
                    ),
                    ValueImportBinding::Namespace {
                        local,
                        local_span,
                        span,
                    } => ImportDeclarationSpecifier::new_import_namespace_specifier(
                        *span,
                        BindingIdentifier::new(*local_span, Ident::from_str_in(local, ast), ast),
                        ast,
                    ),
                    ValueImportBinding::Named(binding) => {
                        ImportDeclarationSpecifier::new_import_specifier(
                            binding.span,
                            binding
                                .imported
                                .to_ast_with_span(ast, binding.imported_span),
                            BindingIdentifier::new(
                                binding.local_span,
                                Ident::from_str_in(&binding.local, ast),
                                ast,
                            ),
                            ImportOrExportKind::Value,
                            ast,
                        )
                    }
                }),
                ast,
            )
        };
        Statement::new_import_declaration(
            SPAN,
            Some(specifiers),
            self.source.to_ast(ast),
            None,
            None,
            import_kind,
            ast,
        )
    }
}

pub(super) struct ReExportSplitEmitter {
    source: ModuleStringLiteral,
    value_bindings: Vec<ExportBinding>,
    type_bindings: Vec<ExportBinding>,
}

pub(super) struct LocalExportSplitEmitter {
    value_bindings: Vec<ExportBinding>,
    type_bindings: Vec<ExportBinding>,
}

impl ReExportSplitEmitter {
    pub(super) fn new(
        source: ModuleStringLiteral,
        value_bindings: Vec<ExportBinding>,
        type_bindings: Vec<ExportBinding>,
    ) -> Self {
        Self {
            source,
            value_bindings,
            type_bindings,
        }
    }

    pub(super) fn statements<'a>(&self, ast: &AstBuilder<'a>) -> ArenaVec<'a, Statement<'a>> {
        let mut statements = ArenaVec::new_in(ast);
        if !self.value_bindings.is_empty() {
            statements.push(self.export_statement(
                ast,
                &self.value_bindings,
                ImportOrExportKind::Value,
            ));
        }
        statements.push(self.export_statement(ast, &self.type_bindings, ImportOrExportKind::Type));
        statements
    }

    fn export_statement<'a>(
        &self,
        ast: &AstBuilder<'a>,
        bindings: &[ExportBinding],
        export_kind: ImportOrExportKind,
    ) -> Statement<'a> {
        let specifiers = ArenaVec::from_iter_in(
            bindings.iter().map(|binding| {
                ExportSpecifier::new(
                    SPAN,
                    binding.local.to_ast(ast),
                    binding.exported.to_ast(ast),
                    ImportOrExportKind::Value,
                    ast,
                )
            }),
            ast,
        );
        Statement::new_export_from_declaration(
            SPAN,
            specifiers,
            self.source.to_ast(ast),
            export_kind,
            None,
            ast,
        )
    }
}

impl LocalExportSplitEmitter {
    pub(super) fn new(
        value_bindings: Vec<ExportBinding>,
        type_bindings: Vec<ExportBinding>,
    ) -> Self {
        Self {
            value_bindings,
            type_bindings,
        }
    }

    pub(super) fn statements<'a>(&self, ast: &AstBuilder<'a>) -> ArenaVec<'a, Statement<'a>> {
        let mut statements = ArenaVec::new_in(ast);
        if !self.value_bindings.is_empty() {
            statements.push(self.export_statement(
                ast,
                &self.value_bindings,
                ImportOrExportKind::Value,
            ));
        }
        statements.push(self.export_statement(ast, &self.type_bindings, ImportOrExportKind::Type));
        statements
    }

    fn export_statement<'a>(
        &self,
        ast: &AstBuilder<'a>,
        bindings: &[ExportBinding],
        export_kind: ImportOrExportKind,
    ) -> Statement<'a> {
        let specifiers = ArenaVec::from_iter_in(
            bindings.iter().map(|binding| {
                ExportSpecifier::new(
                    SPAN,
                    binding.local.to_ast(ast),
                    binding.exported.to_ast(ast),
                    ImportOrExportKind::Value,
                    ast,
                )
            }),
            ast,
        );
        Statement::new_export_named_declaration(SPAN, specifiers, export_kind, ast)
    }
}

impl ModuleStringLiteral {
    pub(super) fn from_string_literal(literal: &StringLiteral<'_>) -> Self {
        Self {
            value: literal.value.to_owned_text(),
            raw: literal.raw.as_ref().map(ToString::to_string),
        }
    }

    pub(super) fn to_ast<'a>(&self, ast: &AstBuilder<'a>) -> StringLiteral<'a> {
        StringLiteral::new(
            SPAN,
            Str::from_str_in(&self.value, ast),
            self.raw.as_ref().map(|raw| Str::from_str_in(raw, ast)),
            ast,
        )
    }
}

impl ModuleBindingName {
    pub(super) fn from_module_export_name(name: &ModuleExportName<'_>) -> Self {
        match name {
            ModuleExportName::IdentifierName(identifier) => {
                Self::Identifier(identifier.name.to_string())
            }
            ModuleExportName::IdentifierReference(identifier) => {
                Self::Identifier(identifier.name.to_string())
            }
            ModuleExportName::StringLiteral(literal) => {
                Self::StringLiteral(ModuleStringLiteral::from_string_literal(literal))
            }
        }
    }

    fn to_ast<'a>(&self, ast: &AstBuilder<'a>) -> ModuleExportName<'a> {
        self.to_ast_with_span(ast, SPAN)
    }

    fn to_ast_with_span<'a>(&self, ast: &AstBuilder<'a>, span: Span) -> ModuleExportName<'a> {
        match self {
            Self::Identifier(name) => {
                ModuleExportName::new_identifier_name(span, Ident::from_str_in(name, ast), ast)
            }
            Self::StringLiteral(literal) => ModuleExportName::StringLiteral(StringLiteral::new(
                span,
                Str::from_str_in(&literal.value, ast),
                literal.raw.as_ref().map(|raw| Str::from_str_in(raw, ast)),
                ast,
            )),
        }
    }
}

impl ExportBinding {
    pub(super) fn from_export_specifier(specifier: &ExportSpecifier<'_>) -> Self {
        Self {
            local: ModuleBindingName::from_module_export_name(&specifier.local),
            exported: ModuleBindingName::from_module_export_name(&specifier.exported),
        }
    }
}
