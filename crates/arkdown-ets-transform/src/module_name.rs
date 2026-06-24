use oxc_ast::ast::ModuleExportName;

pub(crate) struct ModuleExportNameText<'a, 'b> {
    name: &'b ModuleExportName<'a>,
}

impl<'a, 'b> ModuleExportNameText<'a, 'b> {
    pub(crate) fn new(name: &'b ModuleExportName<'a>) -> Self {
        Self { name }
    }

    pub(crate) fn binding_name(&self) -> String {
        match self.name {
            ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
            ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
            ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
        }
    }
}
