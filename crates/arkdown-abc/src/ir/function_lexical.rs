use rustc_hash::{FxHashMap, FxHashSet};

use oxc_ast::ast::{
    ArrowFunctionExpression, BindingPattern, Declaration, ExportDefaultDeclarationKind, Function,
    FunctionBody, MetaProperty, Program, Statement, ThisExpression, VariableDeclaration,
};
use oxc_ast_visit::{walk, Visit};
use oxc_syntax::scope::ScopeFlags;

use crate::ir::ast_analysis::is_new_target_meta;

#[derive(Clone, Debug, Default)]
pub(super) struct FunctionLexicalInfo {
    pub(super) names: Vec<String>,
    pub(super) parent_spans: Vec<(u32, u32)>,
}

pub(super) fn collect_function_lexical_bindings(
    program: &Program<'_>,
) -> FxHashMap<(u32, u32), FunctionLexicalInfo> {
    let mut collector = FunctionLexicalCollector {
        stack: Vec::new(),
        infos: FxHashMap::default(),
    };
    collector.visit_program(program);
    collector.infos
}

struct FunctionFrame {
    span: (u32, u32),
    is_arrow: bool,
    bindings: FxHashSet<String>,
    captures: Vec<String>,
    captures_this: bool,
    captures_new_target: bool,
}

struct FunctionLexicalCollector {
    stack: Vec<FunctionFrame>,
    infos: FxHashMap<(u32, u32), FunctionLexicalInfo>,
}

impl FunctionLexicalCollector {
    fn push_frame(&mut self, span: (u32, u32), is_arrow: bool, bindings: FxHashSet<String>) {
        let parent_spans = self.stack.iter().rev().map(|frame| frame.span).collect();
        self.infos.entry(span).or_insert(FunctionLexicalInfo {
            names: Vec::new(),
            parent_spans,
        });
        self.stack.push(FunctionFrame {
            span,
            is_arrow,
            bindings,
            captures: Vec::new(),
            captures_this: false,
            captures_new_target: false,
        });
    }

    fn pop_frame(&mut self) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        if let Some(info) = self.infos.get_mut(&frame.span) {
            let mut names = frame.captures;
            if frame.captures_this && !names.iter().any(|name| name == "this") {
                names.push("this".to_string());
            }
            if frame.captures_new_target && !names.iter().any(|name| name == "new.target") {
                names.push("new.target".to_string());
            }
            info.names = names;
        }
    }

    fn record_reference(&mut self, name: &str) {
        let Some(current_idx) = self.stack.len().checked_sub(1) else {
            return;
        };
        let Some(owner_idx) = self
            .stack
            .iter()
            .rposition(|frame| frame.bindings.contains(name))
        else {
            return;
        };
        if owner_idx == current_idx {
            return;
        }
        let owner = &mut self.stack[owner_idx];
        if !owner.captures.iter().any(|existing| existing == name) {
            owner.captures.push(name.to_string());
        }
    }

    fn record_arrow_lexical_capture(&mut self, name: &str) {
        let Some(current_idx) = self.stack.len().checked_sub(1) else {
            return;
        };
        if !self.stack[current_idx].is_arrow {
            return;
        }
        let Some(owner_idx) = self.stack[..current_idx]
            .iter()
            .rposition(|frame| !frame.is_arrow)
        else {
            return;
        };
        let owner = &mut self.stack[owner_idx];
        match name {
            "this" => owner.captures_this = true,
            "new.target" => owner.captures_new_target = true,
            _ => {}
        }
    }
}

impl<'a> Visit<'a> for FunctionLexicalCollector {
    fn visit_program(&mut self, it: &Program<'a>) {
        walk::walk_program(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        let bindings = collect_function_bindings(it);
        self.push_frame((it.span.start, it.span.end), false, bindings);
        walk::walk_function(self, it, flags);
        self.pop_frame();
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        let bindings = collect_arrow_bindings(it);
        self.push_frame((it.span.start, it.span.end), true, bindings);
        walk::walk_arrow_function_expression(self, it);
        self.pop_frame();
    }

    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        self.record_reference(it.name.as_str());
    }

    fn visit_this_expression(&mut self, _it: &ThisExpression) {
        self.record_arrow_lexical_capture("this");
    }

    fn visit_meta_property(&mut self, it: &MetaProperty<'a>) {
        if is_new_target_meta(it) {
            self.record_arrow_lexical_capture("new.target");
        }
        walk::walk_meta_property(self, it);
    }
}

fn collect_function_bindings(function: &Function<'_>) -> FxHashSet<String> {
    let mut names = FxHashSet::default();
    if let Some(id) = &function.id {
        names.insert(id.name.as_str().to_string());
    }
    push_param_names(&function.params.items, &mut names);
    if let Some(rest) = &function.params.rest {
        if let BindingPattern::BindingIdentifier(id) = &rest.rest.argument {
            names.insert(id.name.as_str().to_string());
        }
    }
    if let Some(body) = &function.body {
        collect_body_decl_names(body, &mut names);
    }
    names
}

fn collect_arrow_bindings(arrow: &ArrowFunctionExpression<'_>) -> FxHashSet<String> {
    let mut names = FxHashSet::default();
    push_param_names(&arrow.params.items, &mut names);
    if let Some(rest) = &arrow.params.rest {
        if let BindingPattern::BindingIdentifier(id) = &rest.rest.argument {
            names.insert(id.name.as_str().to_string());
        }
    }
    collect_body_decl_names(arrow.body.as_ref(), &mut names);
    names
}

fn push_param_names(params: &[oxc_ast::ast::FormalParameter<'_>], out: &mut FxHashSet<String>) {
    for param in params {
        if let BindingPattern::BindingIdentifier(id) = &param.pattern {
            out.insert(id.name.as_str().to_string());
        }
    }
}

fn collect_body_decl_names(body: &FunctionBody<'_>, out: &mut FxHashSet<String>) {
    for stmt in &body.statements {
        collect_stmt_decl_names(stmt, out);
    }
}

fn collect_stmt_decl_names(stmt: &Statement<'_>, out: &mut FxHashSet<String>) {
    match stmt {
        Statement::BlockStatement(block) => {
            for stmt in &block.body {
                collect_stmt_decl_names(stmt, out);
            }
        }
        Statement::FunctionDeclaration(func) => {
            if let Some(id) = &func.id {
                out.insert(id.name.as_str().to_string());
            }
        }
        Statement::ClassDeclaration(cls) => {
            if let Some(id) = &cls.id {
                out.insert(id.name.as_str().to_string());
            }
        }
        Statement::VariableDeclaration(decl) => push_variable_decl_names(decl, out),
        Statement::IfStatement(stmt) => {
            collect_stmt_decl_names(&stmt.consequent, out);
            if let Some(alternate) = &stmt.alternate {
                collect_stmt_decl_names(alternate, out);
            }
        }
        Statement::WhileStatement(stmt) => collect_stmt_decl_names(&stmt.body, out),
        Statement::DoWhileStatement(stmt) => collect_stmt_decl_names(&stmt.body, out),
        Statement::ForStatement(stmt) => collect_stmt_decl_names(&stmt.body, out),
        Statement::ForInStatement(stmt) => collect_stmt_decl_names(&stmt.body, out),
        Statement::ForOfStatement(stmt) => collect_stmt_decl_names(&stmt.body, out),
        Statement::TryStatement(stmt) => {
            for stmt in &stmt.block.body {
                collect_stmt_decl_names(stmt, out);
            }
            if let Some(handler) = &stmt.handler {
                for stmt in &handler.body.body {
                    collect_stmt_decl_names(stmt, out);
                }
            }
            if let Some(finalizer) = &stmt.finalizer {
                for stmt in &finalizer.body {
                    collect_stmt_decl_names(stmt, out);
                }
            }
        }
        Statement::SwitchStatement(stmt) => {
            for case in &stmt.cases {
                for stmt in &case.consequent {
                    collect_stmt_decl_names(stmt, out);
                }
            }
        }
        Statement::ExportNamedDeclaration(export) => match &export.declaration {
            Some(Declaration::FunctionDeclaration(func)) => {
                if let Some(id) = &func.id {
                    out.insert(id.name.as_str().to_string());
                }
            }
            Some(Declaration::ClassDeclaration(cls)) => {
                if let Some(id) = &cls.id {
                    out.insert(id.name.as_str().to_string());
                }
            }
            Some(Declaration::VariableDeclaration(decl)) => push_variable_decl_names(decl, out),
            _ => {}
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                if let Some(id) = &func.id {
                    out.insert(id.name.as_str().to_string());
                }
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

fn push_variable_decl_names(decl: &VariableDeclaration<'_>, out: &mut FxHashSet<String>) {
    for declarator in &decl.declarations {
        match &declarator.id {
            BindingPattern::BindingIdentifier(id) => {
                out.insert(id.name.as_str().to_string());
            }
            BindingPattern::ArrayPattern(array) => {
                for element in &array.elements {
                    if let Some(BindingPattern::BindingIdentifier(id)) = element {
                        out.insert(id.name.as_str().to_string());
                    }
                }
            }
            BindingPattern::ObjectPattern(object) => {
                for property in &object.properties {
                    if let BindingPattern::BindingIdentifier(id) = &property.value {
                        out.insert(id.name.as_str().to_string());
                    }
                }
            }
            _ => {}
        }
    }
}
