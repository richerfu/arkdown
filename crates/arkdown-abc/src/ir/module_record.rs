use oxc_ast::ast::{
    AssignmentTarget, BindingPattern, Declaration, ExportAllDeclaration,
    ExportDefaultDeclarationKind, Expression, ImportDeclarationSpecifier, ImportOrExportKind,
    ModuleExportName, Program, Statement,
};

use crate::emit::file::{LiteralArray, LiteralEntry};

#[derive(Debug, Default, Clone)]
pub(super) struct ModuleRecordInfo {
    pub(super) module_requests: Vec<String>,
    pub(super) regular_imports: Vec<RegularImportEntry>,
    pub(super) namespace_imports: Vec<NamespaceImportEntry>,
    pub(super) local_exports: Vec<LocalExportEntry>,
    pub(super) indirect_exports: Vec<IndirectExportEntry>,
    pub(super) star_exports: Vec<StarExportEntry>,
}

#[derive(Debug, Clone)]
pub(super) struct RegularImportEntry {
    pub(super) local_name: String,
    pub(super) import_name: String,
    pub(super) module_request_idx: u16,
}

#[derive(Debug, Clone)]
pub(super) struct NamespaceImportEntry {
    pub(super) local_name: String,
    pub(super) module_request_idx: u16,
}

#[derive(Debug, Clone)]
pub(super) struct LocalExportEntry {
    pub(super) local_name: String,
    pub(super) export_name: String,
}

#[derive(Debug, Clone)]
pub(super) struct IndirectExportEntry {
    pub(super) export_name: String,
    pub(super) import_name: String,
    pub(super) module_request_idx: u16,
}

#[derive(Debug, Clone)]
pub(super) struct StarExportEntry {
    pub(super) module_request_idx: u16,
}

impl ModuleRecordInfo {
    fn request_index(&mut self, specifier: &str) -> u16 {
        if let Some(index) = self
            .module_requests
            .iter()
            .position(|request| request == specifier)
        {
            return index as u16;
        }
        let index = self.module_requests.len() as u16;
        self.module_requests.push(specifier.to_string());
        index
    }

    fn add_regular_import(&mut self, local_name: String, specifier: &str) {
        self.add_named_import(local_name, "default".to_string(), specifier);
    }

    fn add_named_import(&mut self, local_name: String, import_name: String, specifier: &str) {
        let module_request_idx = self.request_index(specifier);
        if self.regular_imports.iter().any(|import| {
            import.local_name == local_name && import.module_request_idx == module_request_idx
        }) {
            return;
        }
        self.regular_imports.push(RegularImportEntry {
            local_name,
            import_name,
            module_request_idx,
        });
    }

    fn add_namespace_import(&mut self, local_name: String, specifier: &str) {
        let module_request_idx = self.request_index(specifier);
        if self
            .namespace_imports
            .iter()
            .any(|i| i.local_name == local_name && i.module_request_idx == module_request_idx)
        {
            return;
        }
        self.namespace_imports.push(NamespaceImportEntry {
            local_name,
            module_request_idx,
        });
    }

    fn add_local_export(&mut self, local_name: String, export_name: &str) {
        if self
            .local_exports
            .iter()
            .any(|export| export.local_name == local_name && export.export_name == export_name)
        {
            return;
        }
        self.local_exports.push(LocalExportEntry {
            local_name,
            export_name: export_name.to_string(),
        });
    }

    fn add_indirect_export(&mut self, export_name: String, import_name: String, specifier: &str) {
        let module_request_idx = self.request_index(specifier);
        if self
            .indirect_exports
            .iter()
            .any(|e| e.export_name == export_name && e.module_request_idx == module_request_idx)
        {
            return;
        }
        self.indirect_exports.push(IndirectExportEntry {
            export_name,
            import_name,
            module_request_idx,
        });
    }

    fn add_star_export(&mut self, specifier: &str) {
        let module_request_idx = self.request_index(specifier);
        if self
            .star_exports
            .iter()
            .any(|e| e.module_request_idx == module_request_idx)
        {
            return;
        }
        self.star_exports
            .push(StarExportEntry { module_request_idx });
    }
}

pub(super) fn module_record_literal_array(module_record: &ModuleRecordInfo) -> LiteralArray {
    let mut entries = Vec::new();

    entries.push(LiteralEntry::RawU32(
        module_record.module_requests.len() as u32
    ));
    entries.extend(
        module_record
            .module_requests
            .iter()
            .cloned()
            .map(LiteralEntry::RawString),
    );

    entries.push(LiteralEntry::RawU32(
        module_record.regular_imports.len() as u32
    ));
    for import in &module_record.regular_imports {
        entries.push(LiteralEntry::RawString(import.local_name.clone()));
        entries.push(LiteralEntry::RawString(import.import_name.clone()));
        entries.push(LiteralEntry::RawU16(import.module_request_idx));
    }

    entries.push(LiteralEntry::RawU32(
        module_record.namespace_imports.len() as u32
    ));
    for ns in &module_record.namespace_imports {
        entries.push(LiteralEntry::RawU16(ns.module_request_idx));
        entries.push(LiteralEntry::RawString(ns.local_name.clone()));
    }

    entries.push(LiteralEntry::RawU32(
        module_record.local_exports.len() as u32
    ));
    for export in &module_record.local_exports {
        entries.push(LiteralEntry::RawString(export.local_name.clone()));
        entries.push(LiteralEntry::RawString(export.export_name.clone()));
    }

    entries.push(LiteralEntry::RawU32(
        module_record.indirect_exports.len() as u32
    ));
    for export in &module_record.indirect_exports {
        entries.push(LiteralEntry::RawU16(export.module_request_idx));
        entries.push(LiteralEntry::RawString(export.import_name.clone()));
        entries.push(LiteralEntry::RawString(export.export_name.clone()));
    }

    entries.push(LiteralEntry::RawU32(module_record.star_exports.len() as u32));
    for export in &module_record.star_exports {
        entries.push(LiteralEntry::RawU16(export.module_request_idx));
    }

    LiteralArray { entries }
}

/// Collect libpandafile module-record data from `program`.
///
/// Module mode scans top-level `import`/`export` declarations directly.
/// Non-module compatibility mode also recognises CommonJS patterns such as
/// `var import_X = require("@ohos:...")` and `module.exports = Local`.
pub(super) fn collect_module_record(program: &Program<'_>, _is_module: bool) -> ModuleRecordInfo {
    let mut info = ModuleRecordInfo::default();
    for stmt in &program.body {
        scan_stmt_for_module_record(stmt, &mut info);
    }
    info
}

/// Scan a statement for module import/export patterns.
fn scan_stmt_for_module_record(stmt: &Statement<'_>, info: &mut ModuleRecordInfo) {
    match stmt {
        Statement::ExpressionStatement(es) => {
            scan_expr_for_module_record(&es.expression, info);
        }
        Statement::VariableDeclaration(decl) => {
            for declarator in &decl.declarations {
                let local_name = match &declarator.id {
                    BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
                    _ => None,
                };
                if let Some(init) = &declarator.init {
                    if let (Some(local_name), Some(specifier)) =
                        (local_name, require_specifier(init))
                    {
                        let local_name = normalize_import_local_name(local_name, specifier);
                        info.add_regular_import(local_name, specifier);
                    } else {
                        scan_expr_for_module_record(init, info);
                    }
                }
            }
        }
        Statement::ImportDeclaration(import) => {
            if import.import_kind == ImportOrExportKind::Type {
                return;
            }
            let specifier = import.source.value.as_str();
            if let Some(specifiers) = &import.specifiers {
                for spec in specifiers {
                    match spec {
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => {
                            info.add_regular_import(
                                default.local.name.as_str().to_string(),
                                specifier,
                            );
                        }
                        ImportDeclarationSpecifier::ImportSpecifier(named) => {
                            if named.import_kind == ImportOrExportKind::Type {
                                continue;
                            }
                            info.add_named_import(
                                named.local.name.as_str().to_string(),
                                module_export_name(&named.imported),
                                specifier,
                            );
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(ns) => {
                            info.add_namespace_import(
                                ns.local.name.as_str().to_string(),
                                specifier,
                            );
                        }
                    }
                }
            } else {
                info.request_index(specifier);
            }
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                scan_stmt_for_module_record(s, info);
            }
        }
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                scan_expr_for_module_record(arg, info);
            }
        }
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::ClassDeclaration(cls) => {
                let local_name = cls.id.as_ref().map_or("default", |id| id.name.as_str());
                info.add_local_export(local_name.to_string(), "default");
            }
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                let local_name = func.id.as_ref().map_or("default", |id| id.name.as_str());
                info.add_local_export(local_name.to_string(), "default");
            }
            _ => {
                info.add_local_export("default".to_string(), "default");
            }
        },
        Statement::ExportNamedDeclaration(export) => {
            if export.export_kind == ImportOrExportKind::Type {
                return;
            }
            if let Some(source) = &export.source {
                let specifier = source.value.as_str();
                for spec in &export.specifiers {
                    if spec.export_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    let import_name = module_export_name(&spec.local);
                    let export_name = module_export_name(&spec.exported);
                    info.add_indirect_export(export_name, import_name, specifier);
                }
            } else {
                for spec in &export.specifiers {
                    if spec.export_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    info.add_local_export(
                        module_export_name(&spec.local),
                        &module_export_name(&spec.exported),
                    );
                }
                if let Some(declaration) = &export.declaration {
                    match declaration {
                        Declaration::ClassDeclaration(cls) => {
                            if let Some(id) = &cls.id {
                                info.add_local_export(
                                    id.name.as_str().to_string(),
                                    id.name.as_str(),
                                );
                            }
                        }
                        Declaration::FunctionDeclaration(func) => {
                            if let Some(id) = &func.id {
                                info.add_local_export(
                                    id.name.as_str().to_string(),
                                    id.name.as_str(),
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Statement::ExportAllDeclaration(export) => {
            scan_export_all_for_module_record(export, info);
        }
        _ => {}
    }
}

fn scan_export_all_for_module_record(
    export: &ExportAllDeclaration<'_>,
    info: &mut ModuleRecordInfo,
) {
    if export.export_kind == ImportOrExportKind::Type {
        return;
    }
    let specifier = export.source.value.as_str();
    match &export.exported {
        None => {
            info.add_star_export(specifier);
        }
        Some(exported_name) => {
            let export_name = module_export_name(exported_name);
            info.add_indirect_export(export_name, "*".to_string(), specifier);
        }
    }
}

fn scan_expr_for_module_record(expr: &Expression<'_>, info: &mut ModuleRecordInfo) {
    if let Some(specifier) = require_specifier(expr) {
        info.request_index(specifier);
        return;
    }

    match expr {
        Expression::AssignmentExpression(assign) => {
            if is_module_exports_target(&assign.left) {
                if let Expression::Identifier(id) = &assign.right {
                    info.add_local_export(id.name.as_str().to_string(), "default");
                }
            }
            scan_expr_for_module_record(&assign.right, info);
        }
        Expression::CallExpression(call) => {
            for arg in &call.arguments {
                if let Some(expr) = arg.as_expression() {
                    scan_expr_for_module_record(expr, info);
                }
            }
        }
        Expression::ParenthesizedExpression(expr) => {
            scan_expr_for_module_record(&expr.expression, info);
        }
        Expression::SequenceExpression(seq) => {
            for expr in &seq.expressions {
                scan_expr_for_module_record(expr, info);
            }
        }
        Expression::LogicalExpression(log) => {
            scan_expr_for_module_record(&log.left, info);
            scan_expr_for_module_record(&log.right, info);
        }
        _ => {}
    }
}

fn require_specifier<'a>(expr: &'a Expression<'_>) -> Option<&'a str> {
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    let Expression::Identifier(id) = &call.callee else {
        return None;
    };
    if id.name.as_str() != "require" || call.arguments.len() != 1 {
        return None;
    }
    let Some(Expression::StringLiteral(s)) = call.arguments[0].as_expression() else {
        return None;
    };
    let specifier = s.value.as_str();
    specifier.starts_with("@ohos:").then_some(specifier)
}

fn is_module_exports_target(target: &AssignmentTarget<'_>) -> bool {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return false;
    };
    if member.property.name.as_str() != "exports" {
        return false;
    }
    matches!(&member.object, Expression::Identifier(id) if id.name.as_str() == "module")
}

fn normalize_import_local_name(binding: &str, specifier: &str) -> String {
    if binding.starts_with("_ohos_") {
        return default_import_name(specifier);
    }
    let stripped = binding
        .strip_prefix("import_")
        .or_else(|| binding.strip_prefix("__import_"))
        .unwrap_or(binding);
    if stripped == "default" || stripped.is_empty() {
        return default_import_name(specifier);
    }
    stripped.to_string()
}

fn default_import_name(specifier: &str) -> String {
    specifier
        .rsplit(['.', ':'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(specifier)
        .to_string()
}

fn module_export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(ident) => ident.name.as_str().to_string(),
        ModuleExportName::IdentifierReference(ident) => ident.name.as_str().to_string(),
        ModuleExportName::StringLiteral(lit) => lit.value.as_str().to_string(),
    }
}
