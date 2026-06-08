use rustc_hash::FxHashMap;

use oxc_ast::ast::{
    ArrowFunctionExpression, AssignmentExpression, BindingPattern, Class, Declaration,
    ExportDefaultDeclarationKind, Function, FunctionBody, FunctionType, MetaProperty, Program,
    Statement, ThisExpression, UpdateExpression, VariableDeclaration,
};
use oxc_ast_visit::{walk, Visit};
use oxc_syntax::scope::ScopeFlags;

use crate::ir::assignment_targets::{
    collect_assignment_target_names, collect_simple_assignment_target_names,
};
use crate::ir::ast_analysis::is_new_target_meta;
use crate::ir::class_layout::class_requires_external_instance_initializer;

#[derive(Clone, Debug, Default)]
pub(super) struct FunctionLexicalInfo {
    pub(super) bindings: Vec<FunctionLexicalBinding>,
    pub(super) parent_spans: Vec<(u32, u32)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FunctionLexicalBindingKind {
    Var,
    Function,
    Parameter,
    Lexical,
}

impl FunctionLexicalBindingKind {
    pub(super) fn is_tdz(self) -> bool {
        matches!(self, Self::Lexical)
    }

    pub(super) fn initializes_to_undefined(self) -> bool {
        matches!(self, Self::Var)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FunctionLexicalBinding {
    pub(super) name: String,
    pub(super) kind: FunctionLexicalBindingKind,
}

pub(super) fn collect_function_lexical_bindings(
    program: &Program<'_>,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
) -> FxHashMap<(u32, u32), FunctionLexicalInfo> {
    let mut collector = FunctionLexicalCollector {
        stack: Vec::new(),
        infos: FxHashMap::default(),
        standalone_ecmascript_input,
        use_define_semantic,
    };
    collector.visit_program(program);
    collector.infos
}

struct FunctionFrame {
    span: (u32, u32),
    is_arrow: bool,
    bindings: FxHashMap<String, FunctionLexicalBindingKind>,
    captures: Vec<FunctionLexicalBinding>,
    captures_this: bool,
    captures_new_target: bool,
}

struct FunctionLexicalCollector {
    stack: Vec<FunctionFrame>,
    infos: FxHashMap<(u32, u32), FunctionLexicalInfo>,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
}

impl FunctionLexicalCollector {
    fn push_frame(
        &mut self,
        span: (u32, u32),
        is_arrow: bool,
        bindings: FxHashMap<String, FunctionLexicalBindingKind>,
    ) {
        let parent_spans = self.stack.iter().rev().map(|frame| frame.span).collect();
        self.infos.entry(span).or_insert(FunctionLexicalInfo {
            bindings: Vec::new(),
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
            let mut bindings = frame.captures;
            if frame.captures_new_target
                && !bindings.iter().any(|binding| binding.name == "new.target")
            {
                bindings.push(FunctionLexicalBinding {
                    name: "new.target".to_string(),
                    kind: FunctionLexicalBindingKind::Parameter,
                });
            }
            if frame.captures_this && !bindings.iter().any(|binding| binding.name == "this") {
                bindings.push(FunctionLexicalBinding {
                    name: "this".to_string(),
                    kind: FunctionLexicalBindingKind::Parameter,
                });
            }
            info.bindings = bindings;
        }
    }

    fn record_reference(&mut self, name: &str) {
        let Some(current_idx) = self.stack.len().checked_sub(1) else {
            return;
        };
        let Some((owner_idx, kind)) = self
            .stack
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, frame)| frame.bindings.get(name).copied().map(|kind| (idx, kind)))
        else {
            return;
        };
        if owner_idx == current_idx {
            return;
        }
        let owner = &mut self.stack[owner_idx];
        if !owner.captures.iter().any(|existing| existing.name == name) {
            owner.captures.push(FunctionLexicalBinding {
                name: name.to_string(),
                kind,
            });
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

    fn record_current_scope_capture(&mut self, name: String, kind: FunctionLexicalBindingKind) {
        let Some(frame) = self.stack.last_mut() else {
            return;
        };
        if !frame.captures.iter().any(|existing| existing.name == name) {
            frame.captures.push(FunctionLexicalBinding { name, kind });
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

    fn visit_class(&mut self, it: &Class<'a>) {
        if class_requires_external_instance_initializer(
            &it.body.body,
            it.super_class.is_some(),
            self.standalone_ecmascript_input,
            self.use_define_semantic,
        ) {
            let class_name = it.id.as_ref().map(|id| id.name.as_str()).unwrap_or("_");
            self.record_current_scope_capture(
                format!("#~{class_name}>#instance_initializer"),
                FunctionLexicalBindingKind::Var,
            );
        }
        walk::walk_class(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        self.record_reference(it.name.as_str());
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        let mut names = Vec::new();
        collect_assignment_target_names(&it.left, &mut names);
        for name in names {
            self.record_reference(&name);
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_update_expression(&mut self, it: &UpdateExpression<'a>) {
        let mut names = Vec::new();
        collect_simple_assignment_target_names(&it.argument, &mut names);
        for name in names {
            self.record_reference(&name);
        }
        walk::walk_update_expression(self, it);
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

fn collect_function_bindings(
    function: &Function<'_>,
) -> FxHashMap<String, FunctionLexicalBindingKind> {
    let mut names = FxHashMap::default();
    if function.r#type != FunctionType::FunctionDeclaration {
        if let Some(id) = &function.id {
            names.insert(
                id.name.as_str().to_string(),
                FunctionLexicalBindingKind::Var,
            );
        }
    }
    push_param_names(&function.params.items, &mut names);
    if let Some(rest) = &function.params.rest {
        if let BindingPattern::BindingIdentifier(id) = &rest.rest.argument {
            names.insert(
                id.name.as_str().to_string(),
                FunctionLexicalBindingKind::Parameter,
            );
        }
    }
    if let Some(body) = &function.body {
        collect_body_decl_names(body, &mut names);
    }
    names
}

fn collect_arrow_bindings(
    arrow: &ArrowFunctionExpression<'_>,
) -> FxHashMap<String, FunctionLexicalBindingKind> {
    let mut names = FxHashMap::default();
    push_param_names(&arrow.params.items, &mut names);
    if let Some(rest) = &arrow.params.rest {
        if let BindingPattern::BindingIdentifier(id) = &rest.rest.argument {
            names.insert(
                id.name.as_str().to_string(),
                FunctionLexicalBindingKind::Parameter,
            );
        }
    }
    collect_body_decl_names(arrow.body.as_ref(), &mut names);
    names
}

fn push_param_names(
    params: &[oxc_ast::ast::FormalParameter<'_>],
    out: &mut FxHashMap<String, FunctionLexicalBindingKind>,
) {
    for param in params {
        if let BindingPattern::BindingIdentifier(id) = &param.pattern {
            out.insert(
                id.name.as_str().to_string(),
                FunctionLexicalBindingKind::Parameter,
            );
        }
    }
}

fn collect_body_decl_names(
    body: &FunctionBody<'_>,
    out: &mut FxHashMap<String, FunctionLexicalBindingKind>,
) {
    for stmt in &body.statements {
        collect_stmt_decl_names(stmt, out);
    }
}

fn collect_stmt_decl_names(
    stmt: &Statement<'_>,
    out: &mut FxHashMap<String, FunctionLexicalBindingKind>,
) {
    match stmt {
        Statement::BlockStatement(block) => {
            for stmt in &block.body {
                collect_stmt_decl_names(stmt, out);
            }
        }
        Statement::FunctionDeclaration(func) => {
            if !is_type_only_function(func) {
                if let Some(id) = &func.id {
                    out.insert(
                        id.name.as_str().to_string(),
                        FunctionLexicalBindingKind::Function,
                    );
                }
            }
        }
        Statement::ClassDeclaration(cls) => {
            if let Some(id) = &cls.id {
                out.insert(
                    id.name.as_str().to_string(),
                    FunctionLexicalBindingKind::Lexical,
                );
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
                if !is_type_only_function(func) {
                    if let Some(id) = &func.id {
                        out.insert(
                            id.name.as_str().to_string(),
                            FunctionLexicalBindingKind::Function,
                        );
                    }
                }
            }
            Some(Declaration::ClassDeclaration(cls)) => {
                if let Some(id) = &cls.id {
                    out.insert(
                        id.name.as_str().to_string(),
                        FunctionLexicalBindingKind::Lexical,
                    );
                }
            }
            Some(Declaration::VariableDeclaration(decl)) => push_variable_decl_names(decl, out),
            _ => {}
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                if !is_type_only_function(func) {
                    if let Some(id) = &func.id {
                        out.insert(
                            id.name.as_str().to_string(),
                            FunctionLexicalBindingKind::Function,
                        );
                    }
                }
            }
            ExportDefaultDeclarationKind::ClassDeclaration(cls) => {
                if let Some(id) = &cls.id {
                    out.insert(
                        id.name.as_str().to_string(),
                        FunctionLexicalBindingKind::Lexical,
                    );
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn is_type_only_function(func: &Function<'_>) -> bool {
    func.declare || func.body.is_none()
}

fn push_variable_decl_names(
    decl: &VariableDeclaration<'_>,
    out: &mut FxHashMap<String, FunctionLexicalBindingKind>,
) {
    let kind = if decl.kind == oxc_ast::ast::VariableDeclarationKind::Var {
        FunctionLexicalBindingKind::Var
    } else {
        FunctionLexicalBindingKind::Lexical
    };
    for declarator in &decl.declarations {
        match &declarator.id {
            BindingPattern::BindingIdentifier(id) => {
                out.insert(id.name.as_str().to_string(), kind);
            }
            BindingPattern::ArrayPattern(array) => {
                for element in &array.elements {
                    if let Some(BindingPattern::BindingIdentifier(id)) = element {
                        out.insert(id.name.as_str().to_string(), kind);
                    }
                }
            }
            BindingPattern::ObjectPattern(object) => {
                for property in &object.properties {
                    if let BindingPattern::BindingIdentifier(id) = &property.value {
                        out.insert(id.name.as_str().to_string(), kind);
                    }
                }
            }
            _ => {}
        }
    }
}
