use rustc_hash::{FxHashMap, FxHashSet};

use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, AssignmentTarget, BindingPattern, CallExpression,
    ChainElement, ChainExpression, ClassElement, ComputedMemberExpression, Declaration,
    ExportDefaultDeclarationKind, Expression, Function, MethodDefinition, MethodDefinitionKind,
    ObjectPropertyKind, PrivateFieldExpression, Program, PropertyKey, Statement,
    StaticMemberExpression,
};

use crate::ir::class_layout::{class_needs_static_initializer, ClassStaticInitializationMode};

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
    pub(super) flavor: FunctionRecordFlavor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FunctionRecordFlavor {
    Function,
    ObjectMethod,
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
    standalone_ecmascript_input: bool,
    static_initialization_mode: ClassStaticInitializationMode,
) -> Vec<FunctionRecord<'a>> {
    let mut collector = FunctionRecordCollector {
        names,
        records: Vec::new(),
        seen_spans: FxHashSet::default(),
        standalone_ecmascript_input,
        static_initialization_mode,
        standalone_class_stack: Vec::new(),
        standalone_method_depth: 0,
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
    standalone_ecmascript_input: bool,
    static_initialization_mode: ClassStaticInitializationMode,
    standalone_class_stack: Vec<StandaloneClassFunctionContext>,
    standalone_method_depth: usize,
}

#[derive(Debug)]
struct StandaloneClassFunctionContext {
    class_name: String,
    next_arrow_index: u32,
}

#[derive(Debug, Default)]
pub(super) struct FunctionNameAllocator<'a> {
    normal_names: FxHashSet<String>,
    duplicate_names: FxHashMap<String, u16>,
    top_child_duplicate_indices: FxHashMap<String, u16>,
    scope_name_indices: FxHashMap<String, u32>,
    scope_names: Vec<String>,
    class_scopes: FxHashMap<(u32, u32), ScopeFrame>,
    class_method_names: FxHashMap<(u32, u32), String>,
    default_constructor_names: FxHashMap<(u32, u32), String>,
    static_initializer_names: FxHashMap<(u32, u32), String>,
    scoped_functions: FxHashMap<(u32, u32), ScopedFunction>,
    scope_stack: Vec<ScopeFrame>,
    source: Option<&'a str>,
    use_scope_info_names: bool,
}

#[derive(Clone, Debug, Default)]
struct ScopeFrame {
    full_name: String,
    child_duplicate_indices: FxHashMap<String, u16>,
}

#[derive(Clone, Debug)]
struct ScopedFunction {
    parent_full_name: String,
    tag: char,
    self_name: String,
    duplicate_index: u16,
}

impl<'a> FunctionNameAllocator<'a> {
    pub(super) fn new(source: &'a str, use_scope_info_names: bool) -> Self {
        Self {
            normal_names: FxHashSet::default(),
            duplicate_names: FxHashMap::default(),
            top_child_duplicate_indices: FxHashMap::default(),
            scope_name_indices: FxHashMap::default(),
            scope_names: Vec::new(),
            class_scopes: FxHashMap::default(),
            class_method_names: FxHashMap::default(),
            default_constructor_names: FxHashMap::default(),
            static_initializer_names: FxHashMap::default(),
            scoped_functions: FxHashMap::default(),
            scope_stack: Vec::new(),
            source: Some(source),
            use_scope_info_names,
        }
    }

    pub(super) fn uses_scope_info_names(&self) -> bool {
        self.use_scope_info_names
    }

    pub(super) fn bind(&mut self, start: u32, end: u32, name: Option<&str>) -> String {
        if self.use_scope_info_names {
            return self.bind_scoped_function(start, end, name);
        }

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

    pub(super) fn enter_class_scope(&mut self, span: (u32, u32), name: Option<&str>) -> bool {
        if !self.use_scope_info_names {
            return false;
        }
        let self_name = scope_self_name(name.unwrap_or_default());
        let duplicate_index = self.duplicate_index_for_child('~', &self_name);
        let optimized = self.optimized_self_scope_name(&self_name);
        let full_name = format!(
            "{}~{}{}",
            self.current_scope_path(),
            optimized,
            duplicate_scope_suffix(duplicate_index)
        );
        let frame = ScopeFrame {
            full_name,
            child_duplicate_indices: FxHashMap::default(),
        };
        self.class_scopes.insert(span, frame.clone());
        self.scope_stack.push(frame);
        true
    }

    pub(super) fn exit_class_scope(&mut self, span: (u32, u32)) {
        if let Some(frame) = self.scope_stack.pop() {
            self.class_scopes.insert(span, frame);
        }
    }

    pub(super) fn enter_function_scope(&mut self, span: (u32, u32)) -> bool {
        if !self.use_scope_info_names {
            return false;
        }
        let Some(function) = self.scoped_functions.get(&span).cloned() else {
            return false;
        };
        let optimized = self.optimized_self_scope_name(&function.self_name);
        self.scope_stack.push(ScopeFrame {
            full_name: format!(
                "{}{}{}{}",
                function.parent_full_name,
                function.tag,
                optimized,
                duplicate_scope_suffix(function.duplicate_index)
            ),
            child_duplicate_indices: FxHashMap::default(),
        });
        true
    }

    pub(super) fn exit_function_scope(&mut self) {
        self.scope_stack.pop();
    }

    pub(super) fn bind_class_method(
        &mut self,
        class_span: (u32, u32),
        method: &MethodDefinition<'_>,
        class_name: &str,
    ) -> Option<String> {
        if !self.use_scope_info_names {
            return None;
        }
        let class_scope = self.class_scopes.get(&class_span)?;
        let class_full_name = class_scope.full_name.clone();
        let method_name = class_method_scope_name(method, class_name)?;
        let marker = class_method_scope_tag(method);
        let duplicate_index =
            self.duplicate_index_for_class_child(class_span, marker, &method_name);
        let name = scoped_internal_name(&class_full_name, marker, &method_name, duplicate_index);
        self.class_method_names.insert(
            (method.value.span.start, method.value.span.end),
            name.clone(),
        );
        self.scoped_functions.insert(
            (method.value.span.start, method.value.span.end),
            ScopedFunction {
                parent_full_name: class_full_name,
                tag: marker,
                self_name: method_name,
                duplicate_index,
            },
        );
        Some(name)
    }

    pub(super) fn bind_inferred_class_method(
        &mut self,
        method: &MethodDefinition<'_>,
        class_name: &str,
    ) -> Option<String> {
        if self.use_scope_info_names {
            return None;
        }
        let inferred_name = class_method_scope_name(method, class_name)?;
        let name = self.bind(
            method.value.span.start,
            method.value.span.end,
            Some(&inferred_name),
        );
        self.class_method_names.insert(
            (method.value.span.start, method.value.span.end),
            name.clone(),
        );
        Some(name)
    }

    pub(super) fn class_method_name(&self, start: u32, end: u32) -> Option<&str> {
        self.class_method_names
            .get(&(start, end))
            .map(String::as_str)
    }

    pub(super) fn default_constructor_name(
        &mut self,
        class_span: (u32, u32),
        class_name: &str,
    ) -> Option<String> {
        if let Some(name) = self.default_constructor_names.get(&class_span) {
            return Some(name.clone());
        }
        if !self.use_scope_info_names {
            let name = self.bind(0, 0, Some(class_name));
            self.default_constructor_names
                .insert(class_span, name.clone());
            return Some(name);
        }
        let class_scope = self.class_scopes.get(&class_span)?;
        let class_full_name = class_scope.full_name.clone();
        let class_name = scope_self_name(class_name);
        let duplicate_index = self.duplicate_index_for_class_child(class_span, '=', &class_name);
        let name = scoped_internal_name(&class_full_name, '=', &class_name, duplicate_index);
        self.default_constructor_names
            .insert(class_span, name.clone());
        Some(name)
    }

    pub(super) fn static_initializer_name(
        &mut self,
        class_span: (u32, u32),
        class_name: &str,
    ) -> Option<String> {
        if let Some(name) = self.static_initializer_names.get(&class_span) {
            return Some(name.clone());
        }
        let name = if self.use_scope_info_names {
            let class_scope = self.class_scopes.get(&class_span)?;
            let class_full_name = class_scope.full_name.clone();
            let initializer_name = "static_initializer";
            let duplicate_index =
                self.duplicate_index_for_class_child(class_span, '>', initializer_name);
            scoped_internal_name(&class_full_name, '>', initializer_name, duplicate_index)
        } else {
            format!("#~{class_name}>#static_initializer")
        };
        self.static_initializer_names
            .insert(class_span, name.clone());
        Some(name)
    }

    pub(super) fn class_method_names_by_span(
        &self,
    ) -> impl Iterator<Item = ((u32, u32), String)> + '_ {
        self.class_method_names
            .iter()
            .map(|(span, name)| (*span, name.clone()))
    }

    pub(super) fn static_initializer_names_by_class_span(
        &self,
    ) -> impl Iterator<Item = ((u32, u32), String)> + '_ {
        self.static_initializer_names
            .iter()
            .map(|(span, name)| (*span, name.clone()))
    }

    pub(super) fn default_constructor_names_by_class_span(
        &self,
    ) -> impl Iterator<Item = ((u32, u32), String)> + '_ {
        self.default_constructor_names
            .iter()
            .map(|(span, name)| (*span, name.clone()))
    }

    pub(super) fn scope_names(&self) -> &[String] {
        &self.scope_names
    }

    pub(super) fn enter_commonjs_wrapper_scope(&mut self) -> bool {
        if !self.use_scope_info_names {
            return false;
        }
        self.scope_stack.push(ScopeFrame {
            // Scope frames store the path without the leading `#`. The
            // synthetic CommonJS wrapper method is `#*#`, so children use `*`
            // as their parent path and become names such as `#**#factory`.
            full_name: "*".to_string(),
            child_duplicate_indices: FxHashMap::default(),
        });
        true
    }

    pub(super) fn exit_commonjs_wrapper_scope(&mut self) {
        if self.use_scope_info_names {
            self.scope_stack.pop();
        }
    }

    fn bind_scoped_function(&mut self, start: u32, end: u32, name: Option<&str>) -> String {
        let self_name = scope_self_name(name.unwrap_or_default());
        let parent_full_name = self.current_scope_path();
        let duplicate_index = self.duplicate_index_for_child('*', &self_name);
        self.scoped_functions.insert(
            (start, end),
            ScopedFunction {
                parent_full_name: parent_full_name.clone(),
                tag: '*',
                self_name: self_name.clone(),
                duplicate_index,
            },
        );
        scoped_internal_name(&parent_full_name, '*', &self_name, duplicate_index)
    }

    fn current_scope_path(&self) -> String {
        self.scope_stack
            .last()
            .map_or_else(String::new, |scope| scope.full_name.clone())
    }

    fn optimized_self_scope_name(&mut self, self_name: &str) -> String {
        if let Some(index) = self.scope_name_indices.get(self_name) {
            return format!("@{index:x}");
        }

        let index = self.scope_names.len() as u32;
        let index_scope_name = format!("@{index:x}");
        if self_name.len() > index_scope_name.len() {
            self.scope_name_indices.insert(self_name.to_string(), index);
            self.scope_names.push(self_name.to_string());
            index_scope_name
        } else {
            self_name.to_string()
        }
    }

    fn duplicate_index_for_child(&mut self, tag: char, self_name: &str) -> u16 {
        let key = format!("{tag}{self_name}");
        let children = if let Some(scope) = self.scope_stack.last_mut() {
            &mut scope.child_duplicate_indices
        } else {
            &mut self.top_child_duplicate_indices
        };
        let counter = children.entry(key).or_insert(0);
        let duplicate_index = *counter;
        *counter = counter.saturating_add(1);
        duplicate_index
    }

    fn duplicate_index_for_class_child(
        &mut self,
        class_span: (u32, u32),
        tag: char,
        self_name: &str,
    ) -> u16 {
        let key = format!("{tag}{self_name}");
        if self.scope_stack.last().is_some_and(|scope| {
            self.class_scopes
                .get(&class_span)
                .is_some_and(|class_scope| class_scope.full_name == scope.full_name)
        }) {
            let children = &mut self
                .scope_stack
                .last_mut()
                .expect("checked class scope")
                .child_duplicate_indices;
            let counter = children.entry(key).or_insert(0);
            let duplicate_index = *counter;
            *counter = counter.saturating_add(1);
            return duplicate_index;
        }

        let Some(class_scope) = self.class_scopes.get_mut(&class_span) else {
            return 0;
        };
        let counter = class_scope.child_duplicate_indices.entry(key).or_insert(0);
        let duplicate_index = *counter;
        *counter = counter.saturating_add(1);
        duplicate_index
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
            Statement::ClassDeclaration(class) => {
                self.scan_class_elements(
                    class.id.as_ref().map(|id| id.name.as_str()),
                    (class.span.start, class.span.end),
                    &class.body.body,
                );
            }
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
                    self.scan_class_elements(
                        class
                            .id
                            .as_ref()
                            .map(|id| id.name.as_str())
                            .or(Some("default")),
                        (class.span.start, class.span.end),
                        &class.body.body,
                    );
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
                            self.scan_class_elements(
                                class.id.as_ref().map(|id| id.name.as_str()),
                                (class.span.start, class.span.end),
                                &class.body.body,
                            );
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
                self.scan_arrow_body(arrow);
            }
            Expression::AssignmentExpression(assign) => {
                let inferred = assignment_target_name(&assign.left).or(inferred_name);
                self.scan_expr(&assign.right, inferred);
            }
            Expression::CallExpression(call) => {
                self.scan_call_expression(call);
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
                    let inferred = property_key_name(&property.key);
                    if property.method {
                        if let Expression::FunctionExpression(function) = &property.value {
                            self.push_function_with_flavor(
                                function,
                                inferred,
                                FunctionRecordFlavor::ObjectMethod,
                            );
                            self.scan_function_body(function);
                            continue;
                        }
                    }
                    self.scan_expr(&property.value, inferred);
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
            Expression::AwaitExpression(expr) => self.scan_expr(&expr.argument, None),
            Expression::ImportExpression(expr) => self.scan_expr(&expr.source, None),
            Expression::TaggedTemplateExpression(expr) => self.scan_expr(&expr.tag, None),
            Expression::UnaryExpression(expr) => self.scan_expr(&expr.argument, None),
            Expression::UpdateExpression(_) => {}
            Expression::ParenthesizedExpression(expr) => {
                self.scan_expr(&expr.expression, inferred_name);
            }
            Expression::ChainExpression(chain) => self.scan_chain_expression(chain),
            Expression::ComputedMemberExpression(member) => self.scan_computed_member(member),
            Expression::StaticMemberExpression(member) => self.scan_static_member(member),
            Expression::PrivateFieldExpression(member) => self.scan_private_field(member),
            Expression::ClassExpression(class) => {
                self.scan_class_elements(
                    class
                        .id
                        .as_ref()
                        .map(|id| id.name.as_str())
                        .or(inferred_name),
                    (class.span.start, class.span.end),
                    &class.body.body,
                );
            }
            Expression::TSAsExpression(expr) => self.scan_expr(&expr.expression, inferred_name),
            Expression::TSSatisfiesExpression(expr) => {
                self.scan_expr(&expr.expression, inferred_name)
            }
            Expression::TSNonNullExpression(expr) => {
                self.scan_expr(&expr.expression, inferred_name)
            }
            Expression::TSInstantiationExpression(expr) => {
                self.scan_expr(&expr.expression, inferred_name)
            }
            Expression::TSTypeAssertion(expr) => self.scan_expr(&expr.expression, inferred_name),
            _ => {}
        }
    }

    fn scan_call_expression(&mut self, call: &'a CallExpression<'a>) {
        self.scan_expr(&call.callee, None);
        for argument in &call.arguments {
            self.scan_argument(argument);
        }
    }

    fn scan_chain_expression(&mut self, chain: &'a ChainExpression<'a>) {
        self.scan_chain_element(&chain.expression);
    }

    fn scan_chain_element(&mut self, element: &'a ChainElement<'a>) {
        match element {
            ChainElement::CallExpression(call) => self.scan_call_expression(call),
            ChainElement::TSNonNullExpression(expr) => self.scan_expr(&expr.expression, None),
            ChainElement::ComputedMemberExpression(member) => self.scan_computed_member(member),
            ChainElement::StaticMemberExpression(member) => self.scan_static_member(member),
            ChainElement::PrivateFieldExpression(member) => self.scan_private_field(member),
        }
    }

    fn scan_computed_member(&mut self, member: &'a ComputedMemberExpression<'a>) {
        self.scan_expr(&member.object, None);
        self.scan_expr(&member.expression, None);
    }

    fn scan_static_member(&mut self, member: &'a StaticMemberExpression<'a>) {
        self.scan_expr(&member.object, None);
    }

    fn scan_private_field(&mut self, member: &'a PrivateFieldExpression<'a>) {
        self.scan_expr(&member.object, None);
    }

    fn scan_argument(&mut self, argument: &'a Argument<'a>) {
        if let Some(expr) = argument.as_expression() {
            self.scan_expr(expr, None);
        }
    }

    fn scan_class_elements(
        &mut self,
        class_name: Option<&'a str>,
        class_span: (u32, u32),
        elements: &'a [ClassElement<'a>],
    ) {
        let class_scope_pushed = self.names.enter_class_scope(class_span, class_name);
        let class_name = class_name.unwrap_or("_");
        if !class_has_constructor(elements)
            && (!self.standalone_ecmascript_input || self.names.uses_scope_info_names())
        {
            self.names.default_constructor_name(class_span, class_name);
        }
        if self
            .static_initialization_mode
            .uses_external_initializer_method()
            && class_needs_static_initializer(elements)
        {
            self.names.static_initializer_name(class_span, class_name);
        }

        let pushed_context = if self.standalone_ecmascript_input {
            self.standalone_class_stack
                .push(StandaloneClassFunctionContext {
                    class_name: class_name.to_string(),
                    next_arrow_index: 0,
                });
            true
        } else {
            false
        };
        for element in elements {
            match element {
                ClassElement::MethodDefinition(method) => {
                    if self.names.uses_scope_info_names() {
                        self.names.bind_class_method(class_span, method, class_name);
                    } else if !self.standalone_ecmascript_input {
                        self.names.bind_inferred_class_method(method, class_name);
                    }
                    self.standalone_method_depth = self.standalone_method_depth.saturating_add(1);
                    let method_scope_pushed = self.names.uses_scope_info_names()
                        && method
                            .value
                            .body
                            .as_ref()
                            .is_some_and(|body| stmts_contain_function_like(&body.statements));
                    if method_scope_pushed {
                        self.names
                            .enter_function_scope((method.value.span.start, method.value.span.end));
                    }
                    for stmt in method
                        .value
                        .body
                        .as_ref()
                        .map_or(&[][..], |body| body.statements.as_slice())
                    {
                        self.scan_stmt(stmt);
                    }
                    if method_scope_pushed {
                        self.names.exit_function_scope();
                    }
                    self.standalone_method_depth = self.standalone_method_depth.saturating_sub(1);
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
        if pushed_context {
            self.standalone_class_stack.pop();
        }
        if class_scope_pushed {
            self.names.exit_class_scope(class_span);
        }
    }

    fn scan_function_body(&mut self, function: &'a Function<'a>) {
        if let Some(body) = &function.body {
            let function_scope_pushed =
                self.names.uses_scope_info_names() && stmts_contain_function_like(&body.statements);
            if function_scope_pushed {
                self.names
                    .enter_function_scope((function.span.start, function.span.end));
            }
            for stmt in &body.statements {
                self.scan_stmt(stmt);
            }
            if function_scope_pushed {
                self.names.exit_function_scope();
            }
        }
    }

    fn scan_arrow_body(&mut self, arrow: &'a ArrowFunctionExpression<'a>) {
        let function_scope_pushed = self.names.uses_scope_info_names()
            && stmts_contain_function_like(&arrow.body.statements);
        if function_scope_pushed {
            self.names
                .enter_function_scope((arrow.span.start, arrow.span.end));
        }
        for stmt in &arrow.body.statements {
            self.scan_stmt(stmt);
        }
        if function_scope_pushed {
            self.names.exit_function_scope();
        }
    }

    fn push_function(&mut self, function: &'a Function<'a>, inferred_name: Option<&'a str>) {
        self.push_function_with_flavor(function, inferred_name, FunctionRecordFlavor::Function);
    }

    fn push_function_with_flavor(
        &mut self,
        function: &'a Function<'a>,
        inferred_name: Option<&'a str>,
        flavor: FunctionRecordFlavor,
    ) {
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
            flavor,
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
        let name = if self.names.uses_scope_info_names() {
            self.names
                .bind(arrow.span.start, arrow.span.end, inferred_name)
        } else {
            self.standalone_class_arrow_name(inferred_name)
                .unwrap_or_else(|| {
                    self.names
                        .bind(arrow.span.start, arrow.span.end, inferred_name)
                })
        };
        self.records.push(FunctionRecord {
            name,
            node: FunctionRecordNode::Arrow(arrow),
            flavor: FunctionRecordFlavor::Function,
        });
    }

    fn standalone_class_arrow_name(&mut self, inferred_name: Option<&'a str>) -> Option<String> {
        if !self.standalone_ecmascript_input || self.standalone_method_depth == 0 {
            return None;
        }
        let context = self.standalone_class_stack.last_mut()?;
        let index = context.next_arrow_index;
        context.next_arrow_index = context.next_arrow_index.saturating_add(1);
        let suffix = inferred_name
            .filter(|name| !name.is_empty())
            .map(|name| format!("*#{name}"))
            .unwrap_or_default();
        Some(format!("#~{}>@{index}{suffix}", context.class_name))
    }
}

fn stmts_contain_function_like(stmts: &[Statement<'_>]) -> bool {
    stmts.iter().any(stmt_contains_function_like)
}

fn stmt_contains_function_like(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::BlockStatement(block) => stmts_contain_function_like(&block.body),
        Statement::ExpressionStatement(expr) => expr_contains_function_like(&expr.expression),
        Statement::VariableDeclaration(decl) => decl.declarations.iter().any(|declarator| {
            declarator
                .init
                .as_ref()
                .is_some_and(expr_contains_function_like)
        }),
        Statement::FunctionDeclaration(function) => !is_type_only_function(function),
        Statement::ClassDeclaration(class) => !class.declare,
        Statement::IfStatement(stmt) => {
            expr_contains_function_like(&stmt.test)
                || stmt_contains_function_like(&stmt.consequent)
                || stmt
                    .alternate
                    .as_ref()
                    .is_some_and(|alternate| stmt_contains_function_like(alternate))
        }
        Statement::ReturnStatement(stmt) => stmt
            .argument
            .as_ref()
            .is_some_and(expr_contains_function_like),
        Statement::ThrowStatement(stmt) => expr_contains_function_like(&stmt.argument),
        Statement::WhileStatement(stmt) => {
            expr_contains_function_like(&stmt.test) || stmt_contains_function_like(&stmt.body)
        }
        Statement::DoWhileStatement(stmt) => {
            stmt_contains_function_like(&stmt.body) || expr_contains_function_like(&stmt.test)
        }
        Statement::ForStatement(stmt) => {
            stmt.init.as_ref().is_some_and(|init| match init {
                oxc_ast::ast::ForStatementInit::VariableDeclaration(decl) => {
                    decl.declarations.iter().any(|declarator| {
                        declarator
                            .init
                            .as_ref()
                            .is_some_and(expr_contains_function_like)
                    })
                }
                oxc_ast::ast::ForStatementInit::AssignmentExpression(assign) => {
                    expr_contains_function_like(&assign.right)
                }
                oxc_ast::ast::ForStatementInit::SequenceExpression(seq) => {
                    seq.expressions.iter().any(expr_contains_function_like)
                }
                expr => expr_contains_function_like(expr.to_expression()),
            }) || stmt.test.as_ref().is_some_and(expr_contains_function_like)
                || stmt
                    .update
                    .as_ref()
                    .is_some_and(expr_contains_function_like)
                || stmt_contains_function_like(&stmt.body)
        }
        Statement::ForInStatement(stmt) => {
            expr_contains_function_like(&stmt.right) || stmt_contains_function_like(&stmt.body)
        }
        Statement::ForOfStatement(stmt) => {
            expr_contains_function_like(&stmt.right) || stmt_contains_function_like(&stmt.body)
        }
        Statement::TryStatement(stmt) => {
            stmts_contain_function_like(&stmt.block.body)
                || stmt
                    .handler
                    .as_ref()
                    .is_some_and(|handler| stmts_contain_function_like(&handler.body.body))
                || stmt
                    .finalizer
                    .as_ref()
                    .is_some_and(|finalizer| stmts_contain_function_like(&finalizer.body))
        }
        Statement::SwitchStatement(stmt) => {
            expr_contains_function_like(&stmt.discriminant)
                || stmt.cases.iter().any(|case| {
                    case.test.as_ref().is_some_and(expr_contains_function_like)
                        || stmts_contain_function_like(&case.consequent)
                })
        }
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                !is_type_only_function(function)
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => !class.declare,
            _ => export
                .declaration
                .as_expression()
                .is_some_and(expr_contains_function_like),
        },
        Statement::ExportNamedDeclaration(export) => {
            export
                .declaration
                .as_ref()
                .is_some_and(|declaration| match declaration {
                    Declaration::FunctionDeclaration(function) => !is_type_only_function(function),
                    Declaration::ClassDeclaration(class) => !class.declare,
                    Declaration::VariableDeclaration(decl) => {
                        decl.declarations.iter().any(|declarator| {
                            declarator
                                .init
                                .as_ref()
                                .is_some_and(expr_contains_function_like)
                        })
                    }
                    _ => false,
                })
        }
        _ => false,
    }
}

fn expr_contains_function_like(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::FunctionExpression(function) => !is_type_only_function(function),
        Expression::ArrowFunctionExpression(_) | Expression::ClassExpression(_) => true,
        Expression::AssignmentExpression(assign) => expr_contains_function_like(&assign.right),
        Expression::CallExpression(call) => call_contains_function_like(call),
        Expression::NewExpression(new_expr) => {
            expr_contains_function_like(&new_expr.callee)
                || new_expr
                    .arguments
                    .iter()
                    .any(argument_contains_function_like)
        }
        Expression::ObjectExpression(object) => {
            object.properties.iter().any(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    property.method || expr_contains_function_like(&property.value)
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    expr_contains_function_like(&spread.argument)
                }
            })
        }
        Expression::ArrayExpression(array) => array.elements.iter().any(|element| {
            element
                .as_expression()
                .is_some_and(expr_contains_function_like)
        }),
        Expression::SequenceExpression(seq) => {
            seq.expressions.iter().any(expr_contains_function_like)
        }
        Expression::ConditionalExpression(cond) => {
            expr_contains_function_like(&cond.test)
                || expr_contains_function_like(&cond.consequent)
                || expr_contains_function_like(&cond.alternate)
        }
        Expression::BinaryExpression(expr) => {
            expr_contains_function_like(&expr.left) || expr_contains_function_like(&expr.right)
        }
        Expression::LogicalExpression(expr) => {
            expr_contains_function_like(&expr.left) || expr_contains_function_like(&expr.right)
        }
        Expression::AwaitExpression(expr) => expr_contains_function_like(&expr.argument),
        Expression::ImportExpression(expr) => expr_contains_function_like(&expr.source),
        Expression::TaggedTemplateExpression(expr) => expr_contains_function_like(&expr.tag),
        Expression::UnaryExpression(expr) => expr_contains_function_like(&expr.argument),
        Expression::ParenthesizedExpression(expr) => expr_contains_function_like(&expr.expression),
        Expression::ChainExpression(chain) => chain_contains_function_like(chain),
        Expression::ComputedMemberExpression(member) => {
            expr_contains_function_like(&member.object)
                || expr_contains_function_like(&member.expression)
        }
        Expression::StaticMemberExpression(member) => expr_contains_function_like(&member.object),
        Expression::PrivateFieldExpression(member) => expr_contains_function_like(&member.object),
        Expression::TSAsExpression(expr) => expr_contains_function_like(&expr.expression),
        Expression::TSSatisfiesExpression(expr) => expr_contains_function_like(&expr.expression),
        Expression::TSNonNullExpression(expr) => expr_contains_function_like(&expr.expression),
        Expression::TSInstantiationExpression(expr) => {
            expr_contains_function_like(&expr.expression)
        }
        Expression::TSTypeAssertion(expr) => expr_contains_function_like(&expr.expression),
        _ => false,
    }
}

fn call_contains_function_like(call: &CallExpression<'_>) -> bool {
    expr_contains_function_like(&call.callee)
        || call.arguments.iter().any(argument_contains_function_like)
}

fn chain_contains_function_like(chain: &ChainExpression<'_>) -> bool {
    chain_element_contains_function_like(&chain.expression)
}

fn chain_element_contains_function_like(element: &ChainElement<'_>) -> bool {
    match element {
        ChainElement::CallExpression(call) => call_contains_function_like(call),
        ChainElement::TSNonNullExpression(expr) => expr_contains_function_like(&expr.expression),
        ChainElement::ComputedMemberExpression(member) => {
            expr_contains_function_like(&member.object)
                || expr_contains_function_like(&member.expression)
        }
        ChainElement::StaticMemberExpression(member) => expr_contains_function_like(&member.object),
        ChainElement::PrivateFieldExpression(member) => expr_contains_function_like(&member.object),
    }
}

fn argument_contains_function_like(argument: &Argument<'_>) -> bool {
    argument
        .as_expression()
        .is_some_and(expr_contains_function_like)
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

fn class_has_constructor(elements: &[ClassElement<'_>]) -> bool {
    elements.iter().any(|element| {
        matches!(
            element,
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor
        )
    })
}

fn class_method_scope_tag(method: &MethodDefinition<'_>) -> char {
    if method.kind == MethodDefinitionKind::Constructor {
        '='
    } else if method.r#static {
        '<'
    } else {
        '>'
    }
}

fn class_method_scope_name(method: &MethodDefinition<'_>, class_name: &str) -> Option<String> {
    if method.kind == MethodDefinitionKind::Constructor {
        return Some(class_name.to_string());
    }
    property_key_name(&method.key)
        .map(str::to_string)
        .or_else(|| {
            method.computed.then(|| {
                format!(
                    "#computed_class_method_{}_{}#",
                    method.value.span.start, method.value.span.end
                )
            })
        })
}

fn scope_self_name(name: &str) -> String {
    if name.contains('.') || name.contains('\\') {
        String::new()
    } else {
        name.to_string()
    }
}

fn scoped_internal_name(
    parent_full_name: &str,
    tag: char,
    self_name: &str,
    duplicate_index: u16,
) -> String {
    format!(
        "#{parent_full_name}{tag}#{self_name}{}",
        duplicate_scope_suffix(duplicate_index)
    )
}

fn duplicate_scope_suffix(duplicate_index: u16) -> String {
    if duplicate_index == 0 {
        String::new()
    } else {
        format!("^{duplicate_index:x}")
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
