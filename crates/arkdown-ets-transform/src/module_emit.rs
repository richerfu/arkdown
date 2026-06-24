use oxc_allocator::Vec as ArenaVec;
use oxc_ast::ast::{
    ExportSpecifier, ImportOrExportKind, ModuleExportName, Statement, StringLiteral,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_span::SPAN;

#[derive(Debug)]
pub(super) struct NamedImportBinding {
    pub(super) imported: ModuleBindingName,
    pub(super) local: String,
}

#[derive(Debug)]
pub(super) enum ValueImportBinding {
    Default(String),
    Namespace(String),
    Named(NamedImportBinding),
}

#[derive(Debug)]
pub(super) enum TypeImportBinding {
    Default(String),
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

    pub(super) fn statements<'a>(&self, ast: AstBuilder<'a>) -> ArenaVec<'a, Statement<'a>> {
        let mut statements = ast.vec();
        if !self.value_bindings.is_empty() {
            statements.push(self.import_statement(ast, ImportOrExportKind::Value));
        }
        for statement in self.type_import_statements(ast) {
            statements.push(statement);
        }
        statements
    }

    fn type_import_statements<'a>(&self, ast: AstBuilder<'a>) -> ArenaVec<'a, Statement<'a>> {
        let mut statements = ast.vec();
        let default_binding = self.type_bindings.iter().find_map(|binding| match binding {
            TypeImportBinding::Default(local) => Some(local.as_str()),
            TypeImportBinding::Named(_) => None,
        });
        if let Some(local) = default_binding {
            let specifiers = ast.vec1(ast.import_declaration_specifier_import_default_specifier(
                SPAN,
                ast.binding_identifier(SPAN, ast.str(local)),
            ));
            statements.push(Statement::from(ast.module_declaration_import_declaration(
                SPAN,
                Some(specifiers),
                self.source.to_ast(ast),
                None,
                NONE,
                ImportOrExportKind::Type,
            )));
        }

        let named_bindings = self
            .type_bindings
            .iter()
            .filter_map(|binding| match binding {
                TypeImportBinding::Default(_) => None,
                TypeImportBinding::Named(binding) => Some(binding),
            });
        let specifiers = ast.vec_from_iter(named_bindings.map(|binding| {
            ast.import_declaration_specifier_import_specifier(
                SPAN,
                binding.imported.to_ast(ast),
                ast.binding_identifier(SPAN, ast.str(&binding.local)),
                ImportOrExportKind::Value,
            )
        }));
        if !specifiers.is_empty() {
            statements.push(Statement::from(ast.module_declaration_import_declaration(
                SPAN,
                Some(specifiers),
                self.source.to_ast(ast),
                None,
                NONE,
                ImportOrExportKind::Type,
            )));
        }

        statements
    }

    fn import_statement<'a>(
        &self,
        ast: AstBuilder<'a>,
        import_kind: ImportOrExportKind,
    ) -> Statement<'a> {
        let specifiers = if import_kind.is_type() {
            ast.vec_from_iter(self.type_bindings.iter().map(|binding| match binding {
                TypeImportBinding::Default(local) => {
                    ast.import_declaration_specifier_import_default_specifier(
                        SPAN,
                        ast.binding_identifier(SPAN, ast.str(local)),
                    )
                }
                TypeImportBinding::Named(binding) => {
                    ast.import_declaration_specifier_import_specifier(
                        SPAN,
                        binding.imported.to_ast(ast),
                        ast.binding_identifier(SPAN, ast.str(&binding.local)),
                        ImportOrExportKind::Value,
                    )
                }
            }))
        } else {
            ast.vec_from_iter(self.value_bindings.iter().map(|binding| match binding {
                ValueImportBinding::Default(local) => {
                    ast.import_declaration_specifier_import_default_specifier(
                        SPAN,
                        ast.binding_identifier(SPAN, ast.str(local)),
                    )
                }
                ValueImportBinding::Namespace(local) => {
                    ast.import_declaration_specifier_import_namespace_specifier(
                        SPAN,
                        ast.binding_identifier(SPAN, ast.str(local)),
                    )
                }
                ValueImportBinding::Named(binding) => {
                    ast.import_declaration_specifier_import_specifier(
                        SPAN,
                        binding.imported.to_ast(ast),
                        ast.binding_identifier(SPAN, ast.str(&binding.local)),
                        ImportOrExportKind::Value,
                    )
                }
            }))
        };
        Statement::from(ast.module_declaration_import_declaration(
            SPAN,
            Some(specifiers),
            self.source.to_ast(ast),
            None,
            NONE,
            import_kind,
        ))
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

    pub(super) fn statements<'a>(&self, ast: AstBuilder<'a>) -> ArenaVec<'a, Statement<'a>> {
        let mut statements = ast.vec();
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
        ast: AstBuilder<'a>,
        bindings: &[ExportBinding],
        export_kind: ImportOrExportKind,
    ) -> Statement<'a> {
        let specifiers = ast.vec_from_iter(bindings.iter().map(|binding| {
            ast.export_specifier(
                SPAN,
                binding.local.to_ast(ast),
                binding.exported.to_ast(ast),
                ImportOrExportKind::Value,
            )
        }));
        Statement::from(ast.module_declaration_export_named_declaration(
            SPAN,
            None,
            specifiers,
            Some(self.source.to_ast(ast)),
            export_kind,
            NONE,
        ))
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

    pub(super) fn statements<'a>(&self, ast: AstBuilder<'a>) -> ArenaVec<'a, Statement<'a>> {
        let mut statements = ast.vec();
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
        ast: AstBuilder<'a>,
        bindings: &[ExportBinding],
        export_kind: ImportOrExportKind,
    ) -> Statement<'a> {
        let specifiers = ast.vec_from_iter(bindings.iter().map(|binding| {
            ast.export_specifier(
                SPAN,
                binding.local.to_ast(ast),
                binding.exported.to_ast(ast),
                ImportOrExportKind::Value,
            )
        }));
        Statement::from(ast.module_declaration_export_named_declaration(
            SPAN,
            None,
            specifiers,
            None,
            export_kind,
            NONE,
        ))
    }
}

impl ModuleStringLiteral {
    pub(super) fn from_string_literal(literal: &StringLiteral<'_>) -> Self {
        Self {
            value: literal.value.to_string(),
            raw: literal.raw.as_ref().map(ToString::to_string),
        }
    }

    pub(super) fn to_ast<'a>(&self, ast: AstBuilder<'a>) -> StringLiteral<'a> {
        ast.string_literal(
            SPAN,
            ast.str(&self.value),
            self.raw.as_ref().map(|raw| ast.str(raw)),
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

    fn to_ast<'a>(&self, ast: AstBuilder<'a>) -> ModuleExportName<'a> {
        match self {
            Self::Identifier(name) => ast.module_export_name_identifier_name(SPAN, ast.str(name)),
            Self::StringLiteral(literal) => ModuleExportName::StringLiteral(literal.to_ast(ast)),
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
