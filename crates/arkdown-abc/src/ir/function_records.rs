use rustc_hash::{FxHashMap, FxHashSet};

use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, AssignmentTarget, BindingPattern, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, Function, ObjectPropertyKind, Program, PropertyKey,
    Statement,
};
const FNV_PRIME: u64 = 1_099_511_628_211;
const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;

#[derive(Clone, Copy, Debug)]
pub(super) enum FunctionRecordNode<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
}

#[derive(Clone, Debug)]
pub(super) struct FunctionRecord<'a> {
    pub(super) name: String,
    pub(super) node: FunctionRecordNode<'a>,
}

impl<'a> FunctionRecord<'a> {
    pub(super) fn span(&self) -> (u32, u32) {
        match self.node {
            FunctionRecordNode::Function(function) => (function.span.start, function.span.end),
            FunctionRecordNode::Arrow(arrow) => (arrow.span.start, arrow.span.end),
        }
    }
}

pub(super) fn collect_function_records<'a>(
    program: &'a Program<'a>,
    stmts: &'a [Statement<'a>],
    names: &mut FunctionNameAllocator<'a>,
) -> Vec<FunctionRecord<'a>> {
    let mut collector = FunctionRecordCollector {
        names,
        records: Vec::new(),
        seen_spans: FxHashSet::default(),
    };

    // func_main_0 is represented by the Program scope and is emitted by the caller.
    for stmt in stmts {
        collector.scan_stmt(stmt);
    }

    // `stmts` normally equals `program.body`; keep `program` in the signature to
    // mirror es2panda's program-level function-scope queue and avoid accidental
    // collection from transformed statement slices that do not belong to the AST.
    let _ = program;
    collector.records
}

struct FunctionRecordCollector<'a, 'names> {
    names: &'names mut FunctionNameAllocator<'a>,
    records: Vec<FunctionRecord<'a>>,
    seen_spans: FxHashSet<(u32, u32)>,
}

#[derive(Debug, Default)]
pub(super) struct FunctionNameAllocator<'a> {
    normal_names: FxHashSet<String>,
    duplicate_names: FxHashMap<String, u16>,
    source: Option<&'a str>,
}

impl<'a> FunctionNameAllocator<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self {
            normal_names: FxHashSet::default(),
            duplicate_names: FxHashMap::default(),
            source: Some(source),
        }
    }

    pub(super) fn bind(&mut self, start: u32, end: u32, name: Option<&str>) -> String {
        let name = name.unwrap_or_default();
        let ordinary_name = !name.is_empty() && !name.contains('.') && !name.contains('\\');
        if ordinary_name && self.normal_names.insert(name.to_string()) {
            return name.to_string();
        }

        let source = self.source.unwrap_or_default();
        let hash_input = format!("{}{}", source_slice(source, start, end), name);
        let mut result = format!("#{}#", fnv1a64(&hash_input));
        let duplicate_count = self.duplicate_names.entry(hash_input).or_insert(0);
        if *duplicate_count > 0 {
            result = format!(
                "#{hash}_{}#",
                *duplicate_count,
                hash = &result[1..result.len() - 1]
            );
        }
        *duplicate_count += 1;

        if ordinary_name {
            result.push_str(name);
        }
        result
    }
}

impl<'a, 'names> FunctionRecordCollector<'a, 'names> {
    fn scan_stmt(&mut self, stmt: &'a Statement<'a>) {
        match stmt {
            Statement::BlockStatement(block) => {
                for stmt in &block.body {
                    self.scan_stmt(stmt);
                }
            }
            Statement::ExpressionStatement(expr) => self.scan_expr(&expr.expression, None),
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    let inferred = binding_name(&declarator.id);
                    if let Some(init) = &declarator.init {
                        self.scan_expr(init, inferred);
                    }
                }
            }
            Statement::FunctionDeclaration(function) => {
                let inferred = function.id.as_ref().map(|id| id.name.as_str());
                self.push_function(function, inferred);
                self.scan_function_body(function);
            }
            Statement::ClassDeclaration(class) => self.scan_class_elements(&class.body.body),
            Statement::IfStatement(stmt) => {
                self.scan_expr(&stmt.test, None);
                self.scan_stmt(&stmt.consequent);
                if let Some(alternate) = &stmt.alternate {
                    self.scan_stmt(alternate);
                }
            }
            Statement::ReturnStatement(stmt) => {
                if let Some(argument) = &stmt.argument {
                    self.scan_expr(argument, None);
                }
            }
            Statement::ThrowStatement(stmt) => self.scan_expr(&stmt.argument, None),
            Statement::WhileStatement(stmt) => {
                self.scan_expr(&stmt.test, None);
                self.scan_stmt(&stmt.body);
            }
            Statement::DoWhileStatement(stmt) => {
                self.scan_stmt(&stmt.body);
                self.scan_expr(&stmt.test, None);
            }
            Statement::ForStatement(stmt) => {
                if let Some(init) = &stmt.init {
                    match init {
                        oxc_ast::ast::ForStatementInit::VariableDeclaration(decl) => {
                            for declarator in &decl.declarations {
                                let inferred = binding_name(&declarator.id);
                                if let Some(init) = &declarator.init {
                                    self.scan_expr(init, inferred);
                                }
                            }
                        }
                        oxc_ast::ast::ForStatementInit::AssignmentExpression(assign) => {
                            self.scan_expr(&assign.right, assignment_target_name(&assign.left));
                        }
                        oxc_ast::ast::ForStatementInit::SequenceExpression(seq) => {
                            for expr in &seq.expressions {
                                self.scan_expr(expr, None);
                            }
                        }
                        expr => self.scan_expr(expr.to_expression(), None),
                    }
                }
                if let Some(test) = &stmt.test {
                    self.scan_expr(test, None);
                }
                if let Some(update) = &stmt.update {
                    self.scan_expr(update, None);
                }
                self.scan_stmt(&stmt.body);
            }
            Statement::ForInStatement(stmt) => {
                self.scan_expr(&stmt.right, None);
                self.scan_stmt(&stmt.body);
            }
            Statement::ForOfStatement(stmt) => {
                self.scan_expr(&stmt.right, None);
                self.scan_stmt(&stmt.body);
            }
            Statement::TryStatement(stmt) => {
                for stmt in &stmt.block.body {
                    self.scan_stmt(stmt);
                }
                if let Some(handler) = &stmt.handler {
                    for stmt in &handler.body.body {
                        self.scan_stmt(stmt);
                    }
                }
                if let Some(finalizer) = &stmt.finalizer {
                    for stmt in &finalizer.body {
                        self.scan_stmt(stmt);
                    }
                }
            }
            Statement::SwitchStatement(stmt) => {
                self.scan_expr(&stmt.discriminant, None);
                for case in &stmt.cases {
                    if let Some(test) = &case.test {
                        self.scan_expr(test, None);
                    }
                    for stmt in &case.consequent {
                        self.scan_stmt(stmt);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                    let inferred = function
                        .id
                        .as_ref()
                        .map_or(Some("default"), |id| Some(id.name.as_str()));
                    self.push_function(function, inferred);
                    self.scan_function_body(function);
                }
                ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                    self.scan_class_elements(&class.body.body);
                }
                _ => {
                    if let Some(expr) = export.declaration.as_expression() {
                        self.scan_expr(expr, Some("default"));
                    }
                }
            },
            Statement::ExportNamedDeclaration(export) => {
                if let Some(declaration) = &export.declaration {
                    match declaration {
                        Declaration::FunctionDeclaration(function) => {
                            let inferred = function.id.as_ref().map(|id| id.name.as_str());
                            self.push_function(function, inferred);
                            self.scan_function_body(function);
                        }
                        Declaration::ClassDeclaration(class) => {
                            self.scan_class_elements(&class.body.body);
                        }
                        Declaration::VariableDeclaration(decl) => {
                            for declarator in &decl.declarations {
                                let inferred = binding_name(&declarator.id);
                                if let Some(init) = &declarator.init {
                                    self.scan_expr(init, inferred);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn scan_expr(&mut self, expr: &'a Expression<'a>, inferred_name: Option<&'a str>) {
        match expr {
            Expression::FunctionExpression(function) => {
                let inferred = function
                    .id
                    .as_ref()
                    .map(|id| id.name.as_str())
                    .or(inferred_name);
                self.push_function(function, inferred);
                self.scan_function_body(function);
            }
            Expression::ArrowFunctionExpression(arrow) => {
                self.push_arrow(arrow, inferred_name);
                for stmt in &arrow.body.statements {
                    self.scan_stmt(stmt);
                }
            }
            Expression::AssignmentExpression(assign) => {
                let inferred = assignment_target_name(&assign.left).or(inferred_name);
                self.scan_expr(&assign.right, inferred);
            }
            Expression::CallExpression(call) => {
                self.scan_expr(&call.callee, None);
                for argument in &call.arguments {
                    self.scan_argument(argument);
                }
            }
            Expression::NewExpression(new_expr) => {
                self.scan_expr(&new_expr.callee, None);
                for argument in &new_expr.arguments {
                    self.scan_argument(argument);
                }
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        continue;
                    };
                    self.scan_expr(&property.value, property_key_name(&property.key));
                }
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    if let Some(expr) = element.as_expression() {
                        self.scan_expr(expr, None);
                    }
                }
            }
            Expression::SequenceExpression(seq) => {
                for expr in &seq.expressions {
                    self.scan_expr(expr, None);
                }
            }
            Expression::ConditionalExpression(cond) => {
                self.scan_expr(&cond.test, None);
                self.scan_expr(&cond.consequent, inferred_name);
                self.scan_expr(&cond.alternate, inferred_name);
            }
            Expression::BinaryExpression(expr) => {
                self.scan_expr(&expr.left, None);
                self.scan_expr(&expr.right, None);
            }
            Expression::LogicalExpression(expr) => {
                self.scan_expr(&expr.left, None);
                self.scan_expr(&expr.right, None);
            }
            Expression::UnaryExpression(expr) => self.scan_expr(&expr.argument, None),
            Expression::UpdateExpression(_) => {}
            Expression::ParenthesizedExpression(expr) => {
                self.scan_expr(&expr.expression, inferred_name);
            }
            Expression::ChainExpression(_) => {}
            Expression::ClassExpression(class) => self.scan_class_elements(&class.body.body),
            Expression::TSAsExpression(expr) => self.scan_expr(&expr.expression, inferred_name),
            Expression::TSSatisfiesExpression(expr) => {
                self.scan_expr(&expr.expression, inferred_name)
            }
            Expression::TSNonNullExpression(expr) => {
                self.scan_expr(&expr.expression, inferred_name)
            }
            Expression::TSTypeAssertion(expr) => self.scan_expr(&expr.expression, inferred_name),
            _ => {}
        }
    }

    fn scan_argument(&mut self, argument: &'a Argument<'a>) {
        if let Some(expr) = argument.as_expression() {
            self.scan_expr(expr, None);
        }
    }

    fn scan_class_elements(&mut self, elements: &'a [ClassElement<'a>]) {
        for element in elements {
            match element {
                ClassElement::MethodDefinition(method) => {
                    for stmt in method
                        .value
                        .body
                        .as_ref()
                        .map_or(&[][..], |body| body.statements.as_slice())
                    {
                        self.scan_stmt(stmt);
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    if let Some(value) = &property.value {
                        self.scan_expr(value, property_key_name(&property.key));
                    }
                }
                ClassElement::StaticBlock(block) => {
                    for stmt in &block.body {
                        self.scan_stmt(stmt);
                    }
                }
                _ => {}
            }
        }
    }

    fn scan_function_body(&mut self, function: &'a Function<'a>) {
        if let Some(body) = &function.body {
            for stmt in &body.statements {
                self.scan_stmt(stmt);
            }
        }
    }

    fn push_function(&mut self, function: &'a Function<'a>, inferred_name: Option<&'a str>) {
        if is_type_only_function(function) {
            return;
        }
        let span = (function.span.start, function.span.end);
        if !self.seen_spans.insert(span) {
            return;
        }
        let explicit_name = function.id.as_ref().map(|id| id.name.as_str());
        let name = self.names.bind(
            function.span.start,
            function.span.end,
            explicit_name.or(inferred_name),
        );
        self.records.push(FunctionRecord {
            name,
            node: FunctionRecordNode::Function(function),
        });
    }

    fn push_arrow(
        &mut self,
        arrow: &'a ArrowFunctionExpression<'a>,
        inferred_name: Option<&'a str>,
    ) {
        let span = (arrow.span.start, arrow.span.end);
        if !self.seen_spans.insert(span) {
            return;
        }
        let name = self
            .names
            .bind(arrow.span.start, arrow.span.end, inferred_name);
        self.records.push(FunctionRecord {
            name,
            node: FunctionRecordNode::Arrow(arrow),
        });
    }
}

fn is_type_only_function(function: &Function<'_>) -> bool {
    function.declare || function.body.is_none()
}

fn binding_name<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a str> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn assignment_target_name<'a>(target: &'a AssignmentTarget<'a>) -> Option<&'a str> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => Some(identifier.name.as_str()),
        AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        AssignmentTarget::PrivateFieldExpression(member) => Some(member.field.name.as_str()),
        _ => None,
    }
}

fn property_key_name<'a>(key: &'a PropertyKey<'a>) -> Option<&'a str> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn source_slice(source: &str, start: u32, end: u32) -> &str {
    source.get(start as usize..end as usize).unwrap_or_default()
}

fn fnv1a64(value: &str) -> u64 {
    let mut result = FNV_OFFSET;
    for byte in value.as_bytes() {
        result ^= u64::from(*byte);
        result = result.wrapping_mul(FNV_PRIME);
    }
    result
}
