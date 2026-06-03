use rustc_hash::FxHashSet;

use oxc_ast::ast::{
    BindingPattern, Declaration, ExportDefaultDeclarationKind, Function, Program, Statement,
    VariableDeclaration,
};
use oxc_ast_visit::{walk, Visit};
use oxc_syntax::scope::ScopeFlags;

use crate::ir::module_record::ModuleRecordInfo;

pub(super) fn collect_module_lexical_bindings(
    program: &Program<'_>,
    _is_module: bool,
    module_record: &ModuleRecordInfo,
) -> Vec<String> {
    let top_level_bindings = collect_top_level_binding_names(program);
    if top_level_bindings.is_empty() {
        return Vec::new();
    }

    let local_exports: FxHashSet<String> = module_record
        .local_exports
        .iter()
        .map(|export| export.local_name.clone())
        .collect();

    let mut collector = NestedRefCollector {
        function_depth: 0,
        top_level_bindings,
        local_exports,
        names: Vec::new(),
    };
    collector.visit_program(program);
    collector.names
}

fn collect_top_level_binding_names(program: &Program<'_>) -> FxHashSet<String> {
    let mut names = FxHashSet::default();
    for stmt in program.body.as_slice() {
        collect_top_level_decl_names(stmt, &mut names);
    }
    names
}

struct NestedRefCollector {
    function_depth: usize,
    top_level_bindings: FxHashSet<String>,
    local_exports: FxHashSet<String>,
    names: Vec<String>,
}

impl<'a> Visit<'a> for NestedRefCollector {
    fn visit_program(&mut self, it: &Program<'a>) {
        walk::walk_program(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.function_depth += 1;
        walk::walk_function(self, it, flags);
        self.function_depth -= 1;
    }

    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        if self.function_depth == 0 {
            return;
        }
        let name = it.name.as_str();
        if !self.top_level_bindings.contains(name) || self.local_exports.contains(name) {
            return;
        }
        if !self.names.iter().any(|existing| existing == name) {
            self.names.push(name.to_string());
        }
    }
}

fn collect_top_level_decl_names(stmt: &Statement<'_>, out: &mut FxHashSet<String>) {
    match stmt {
        Statement::FunctionDeclaration(func) => push_function_name(func, out),
        Statement::ClassDeclaration(cls) => {
            if let Some(id) = &cls.id {
                out.insert(id.name.as_str().to_string());
            }
        }
        Statement::VariableDeclaration(decl) => push_variable_decl_names(decl, out),
        Statement::TSEnumDeclaration(decl) => {
            out.insert(decl.id.name.as_str().to_string());
        }
        Statement::ExportNamedDeclaration(export) => match &export.declaration {
            Some(Declaration::FunctionDeclaration(func)) => push_function_name(func, out),
            Some(Declaration::ClassDeclaration(cls)) => {
                if let Some(id) = &cls.id {
                    out.insert(id.name.as_str().to_string());
                }
            }
            Some(Declaration::VariableDeclaration(decl)) => push_variable_decl_names(decl, out),
            Some(Declaration::TSEnumDeclaration(decl)) => {
                out.insert(decl.id.name.as_str().to_string());
            }
            _ => {}
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                push_function_name(func, out)
            }
            ExportDefaultDeclarationKind::ClassDeclaration(cls) => {
                if let Some(id) = &cls.id {
                    out.insert(id.name.as_str().to_string());
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn push_function_name(func: &Function<'_>, out: &mut FxHashSet<String>) {
    if is_type_only_function(func) {
        return;
    }
    if let Some(id) = &func.id {
        out.insert(id.name.as_str().to_string());
    }
}

fn is_type_only_function(func: &Function<'_>) -> bool {
    func.declare || func.body.is_none()
}

fn push_variable_decl_names(decl: &VariableDeclaration<'_>, out: &mut FxHashSet<String>) {
    for declarator in &decl.declarations {
        if let BindingPattern::BindingIdentifier(id) = &declarator.id {
            out.insert(id.name.as_str().to_string());
        }
    }
}
