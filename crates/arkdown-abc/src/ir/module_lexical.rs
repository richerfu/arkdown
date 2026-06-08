use rustc_hash::{FxHashMap, FxHashSet};

use oxc_ast::ast::{
    ArrowFunctionExpression, AssignmentExpression, BindingPattern, Class, ClassElement,
    Declaration, ExportDefaultDeclarationKind, Function, Program, Statement, UpdateExpression,
    VariableDeclaration,
};
use oxc_ast_visit::{walk, Visit};
use oxc_syntax::scope::ScopeFlags;

use crate::ir::assignment_targets::{
    collect_assignment_target_names, collect_simple_assignment_target_names,
};
use crate::ir::class_layout::{
    class_needs_static_initializer, class_requires_external_instance_initializer,
    ClassStaticInitializationMode,
};
use crate::ir::module_record::{ModuleRecordInfo, DEFAULT_LOCAL_NAME};

#[derive(Clone, Debug, Default)]
pub(super) struct ScopedClassLexicalLayout {
    class_names_by_binding: FxHashMap<String, Vec<String>>,
    function_class_bindings_by_span: FxHashMap<(u32, u32), String>,
    scoped_names: FxHashSet<String>,
    module_visible_names: FxHashSet<String>,
}

impl ScopedClassLexicalLayout {
    pub(super) fn class_names(&self, class_binding: &str) -> &[String] {
        self.class_names_by_binding
            .get(class_binding)
            .map_or(&[], Vec::as_slice)
    }

    pub(super) fn function_class_names(&self, span: (u32, u32)) -> Option<&[String]> {
        let class_binding = self.function_class_bindings_by_span.get(&span)?;
        Some(self.class_names(class_binding))
    }

    pub(super) fn contains_name(&self, name: &str) -> bool {
        self.scoped_names.contains(name)
    }

    fn keeps_module_name(&self, name: &str) -> bool {
        self.module_visible_names.contains(name)
    }
}

pub(super) fn collect_module_lexical_bindings(
    program: &Program<'_>,
    _is_module: bool,
    module_record: &ModuleRecordInfo,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
) -> Vec<String> {
    let top_level_bindings = collect_top_level_binding_names(program);
    collect_top_level_lexical_bindings(
        program,
        top_level_bindings,
        module_record,
        standalone_ecmascript_input,
        use_define_semantic,
    )
}

pub(super) fn collect_commonjs_wrapper_lexical_bindings(
    program: &Program<'_>,
    wrapper_params: &[&str],
    use_define_semantic: bool,
) -> Vec<String> {
    let mut top_level_bindings = collect_top_level_binding_names(program);
    top_level_bindings.extend(wrapper_params.iter().map(|name| (*name).to_string()));
    collect_top_level_lexical_bindings(
        program,
        top_level_bindings,
        &ModuleRecordInfo::default(),
        false,
        use_define_semantic,
    )
}

pub(super) fn split_scoped_class_lexical_bindings(
    program: &Program<'_>,
    lexical_bindings: Vec<String>,
    module_record: &ModuleRecordInfo,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
    static_initialization_mode: ClassStaticInitializationMode,
) -> (Vec<String>, ScopedClassLexicalLayout) {
    if lexical_bindings.is_empty() {
        return (lexical_bindings, ScopedClassLexicalLayout::default());
    }

    let lexical_set: FxHashSet<String> = lexical_bindings.iter().cloned().collect();
    let local_exports: FxHashSet<String> = module_record
        .local_exports
        .iter()
        .map(|export| export.local_name.clone())
        .collect();
    let module_visible_class_refs =
        collect_module_visible_class_references(program, &lexical_set, &local_exports);
    let mut layout = ScopedClassLexicalLayout::default();

    for stmt in program.body.as_slice() {
        collect_stmt_scoped_class_layout(
            stmt,
            &lexical_set,
            &local_exports,
            &module_visible_class_refs,
            standalone_ecmascript_input,
            use_define_semantic,
            static_initialization_mode,
            &mut layout,
        );
    }

    let module_names = lexical_bindings
        .into_iter()
        .filter(|name| !layout.contains_name(name) || layout.keeps_module_name(name))
        .collect();
    (module_names, layout)
}

fn collect_top_level_lexical_bindings(
    program: &Program<'_>,
    top_level_bindings: FxHashSet<String>,
    module_record: &ModuleRecordInfo,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
) -> Vec<String> {
    let names = collect_top_level_class_environment_names(
        program,
        standalone_ecmascript_input,
        use_define_semantic,
    );
    if top_level_bindings.is_empty() && names.is_empty() {
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
        class_stack: Vec::new(),
        names,
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

fn collect_stmt_scoped_class_layout(
    stmt: &Statement<'_>,
    lexical_set: &FxHashSet<String>,
    local_exports: &FxHashSet<String>,
    module_visible_class_refs: &FxHashSet<String>,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
    static_initialization_mode: ClassStaticInitializationMode,
    layout: &mut ScopedClassLexicalLayout,
) {
    match stmt {
        Statement::ClassDeclaration(cls) => {
            if !cls.declare {
                if let Some(id) = &cls.id {
                    collect_class_scoped_layout(
                        id.name.as_str(),
                        cls,
                        lexical_set,
                        local_exports,
                        module_visible_class_refs,
                        standalone_ecmascript_input,
                        use_define_semantic,
                        static_initialization_mode,
                        layout,
                    );
                }
            }
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(Declaration::ClassDeclaration(cls)) = &export.declaration {
                if !cls.declare {
                    if let Some(id) = &cls.id {
                        collect_class_scoped_layout(
                            id.name.as_str(),
                            cls,
                            lexical_set,
                            local_exports,
                            module_visible_class_refs,
                            standalone_ecmascript_input,
                            use_define_semantic,
                            static_initialization_mode,
                            layout,
                        );
                    }
                }
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::ClassDeclaration(cls) = &export.declaration {
                if !cls.declare {
                    let binding = cls
                        .id
                        .as_ref()
                        .map_or(DEFAULT_LOCAL_NAME, |id| id.name.as_str());
                    collect_class_scoped_layout(
                        binding,
                        cls,
                        lexical_set,
                        local_exports,
                        module_visible_class_refs,
                        standalone_ecmascript_input,
                        use_define_semantic,
                        static_initialization_mode,
                        layout,
                    );
                }
            }
        }
        Statement::VariableDeclaration(decl) => {
            if decl.declare {
                return;
            }
            for declarator in &decl.declarations {
                let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                    continue;
                };
                let Some(oxc_ast::ast::Expression::ClassExpression(cls)) = &declarator.init else {
                    continue;
                };
                let binding = cls
                    .id
                    .as_ref()
                    .map_or(id.name.as_str(), |id| id.name.as_str());
                collect_class_scoped_layout(
                    binding,
                    cls,
                    lexical_set,
                    local_exports,
                    module_visible_class_refs,
                    standalone_ecmascript_input,
                    use_define_semantic,
                    static_initialization_mode,
                    layout,
                );
            }
        }
        _ => {}
    }
}

fn collect_class_scoped_layout(
    class_binding: &str,
    cls: &Class<'_>,
    lexical_set: &FxHashSet<String>,
    local_exports: &FxHashSet<String>,
    module_visible_class_refs: &FxHashSet<String>,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
    static_initialization_mode: ClassStaticInitializationMode,
    layout: &mut ScopedClassLexicalLayout,
) {
    let mut names = Vec::new();
    if module_visible_class_refs.contains(class_binding) {
        layout
            .module_visible_names
            .insert(class_binding.to_string());
    } else {
        push_unique_if_present(&mut names, lexical_set, class_binding);
    }

    let has_external_static_initializer = static_initialization_mode
        .uses_external_initializer_method()
        && class_needs_static_initializer(&cls.body.body);
    if has_external_static_initializer
        && lexical_set.contains(class_binding)
        && !local_exports.contains(class_binding)
    {
        layout
            .module_visible_names
            .insert(class_binding.to_string());
    }

    if class_requires_external_instance_initializer(
        &cls.body.body,
        cls.super_class.is_some(),
        standalone_ecmascript_input,
        use_define_semantic,
    ) {
        let class_name = cls.id.as_ref().map_or(class_binding, |id| id.name.as_str());
        push_unique_if_present(
            &mut names,
            lexical_set,
            &format!("#~{class_name}>#instance_initializer"),
        );
    }

    if names.is_empty() {
        return;
    }

    layout
        .scoped_names
        .extend(names.iter().map(|name| name.to_string()));
    layout
        .class_names_by_binding
        .insert(class_binding.to_string(), names);

    let mut function_collector = ClassFunctionSpanCollector {
        class_binding: class_binding.to_string(),
        function_class_bindings_by_span: &mut layout.function_class_bindings_by_span,
    };
    function_collector.visit_class(cls);
}

fn push_unique_if_present(out: &mut Vec<String>, lexical_set: &FxHashSet<String>, name: &str) {
    if lexical_set.contains(name) && !out.iter().any(|existing| existing == name) {
        out.push(name.to_string());
    }
}

struct ClassFunctionSpanCollector<'a> {
    class_binding: String,
    function_class_bindings_by_span: &'a mut FxHashMap<(u32, u32), String>,
}

impl<'a> Visit<'a> for ClassFunctionSpanCollector<'_> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.function_class_bindings_by_span
            .insert((it.span.start, it.span.end), self.class_binding.clone());
        walk::walk_function(self, it, flags);
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.function_class_bindings_by_span
            .insert((it.span.start, it.span.end), self.class_binding.clone());
        walk::walk_arrow_function_expression(self, it);
    }
}

struct NestedRefCollector {
    function_depth: usize,
    top_level_bindings: FxHashSet<String>,
    local_exports: FxHashSet<String>,
    class_stack: Vec<String>,
    names: Vec<String>,
}

struct TopLevelClassEnvironmentNameCollector {
    function_depth: usize,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
    names: Vec<String>,
}

impl<'a> Visit<'a> for TopLevelClassEnvironmentNameCollector {
    fn visit_program(&mut self, it: &Program<'a>) {
        walk::walk_program(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.function_depth += 1;
        walk::walk_function(self, it, flags);
        self.function_depth -= 1;
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.function_depth += 1;
        walk::walk_arrow_function_expression(self, it);
        self.function_depth -= 1;
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        if self.function_depth == 0 {
            let class_name = it.id.as_ref().map(|id| id.name.as_str()).unwrap_or("_");
            if class_requires_external_instance_initializer(
                &it.body.body,
                it.super_class.is_some(),
                self.standalone_ecmascript_input,
                self.use_define_semantic,
            ) {
                let name = format!("#~{class_name}>#instance_initializer");
                if !self.names.iter().any(|existing| existing == &name) {
                    self.names.push(name);
                }
            }
        }
        walk::walk_class(self, it);
    }
}

fn collect_top_level_class_environment_names(
    program: &Program<'_>,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
) -> Vec<String> {
    let mut collector = TopLevelClassEnvironmentNameCollector {
        function_depth: 0,
        standalone_ecmascript_input,
        use_define_semantic,
        names: Vec::new(),
    };
    collector.visit_program(program);
    collector.names
}

fn collect_module_visible_class_references(
    program: &Program<'_>,
    lexical_set: &FxHashSet<String>,
    local_exports: &FxHashSet<String>,
) -> FxHashSet<String> {
    let mut collector = ModuleVisibleClassReferenceCollector {
        function_depth: 0,
        lexical_set,
        local_exports,
        class_stack: Vec::new(),
        names: FxHashSet::default(),
    };
    collector.visit_program(program);
    collector.names
}

struct ModuleVisibleClassReferenceCollector<'a> {
    function_depth: usize,
    lexical_set: &'a FxHashSet<String>,
    local_exports: &'a FxHashSet<String>,
    class_stack: Vec<String>,
    names: FxHashSet<String>,
}

impl<'a> Visit<'a> for ModuleVisibleClassReferenceCollector<'_> {
    fn visit_program(&mut self, it: &Program<'a>) {
        walk::walk_program(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.function_depth += 1;
        walk::walk_function(self, it, flags);
        self.function_depth -= 1;
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.function_depth += 1;
        walk::walk_arrow_function_expression(self, it);
        self.function_depth -= 1;
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        let class_name = (self.function_depth == 0)
            .then(|| it.id.as_ref().map(|id| id.name.as_str().to_string()))
            .flatten();
        if let Some(name) = class_name {
            self.class_stack.push(name);
            walk::walk_class(self, it);
            self.class_stack.pop();
            return;
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
}

impl ModuleVisibleClassReferenceCollector<'_> {
    fn record_reference(&mut self, name: &str) {
        if self.function_depth == 0 || !self.lexical_set.contains(name) {
            return;
        }
        if self.local_exports.contains(name) {
            return;
        }
        if self
            .class_stack
            .last()
            .is_some_and(|class_name| class_name == name)
        {
            return;
        }
        self.names.insert(name.to_string());
    }
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

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.function_depth += 1;
        walk::walk_arrow_function_expression(self, it);
        self.function_depth -= 1;
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        let class_name = (self.function_depth == 0)
            .then(|| it.id.as_ref().map(|id| id.name.as_str().to_string()))
            .flatten();
        if self.function_depth == 0 {
            for element in &it.body.body {
                match element {
                    ClassElement::PropertyDefinition(property) if property.r#static => {
                        if let Some(value) = property.value.as_ref() {
                            self.function_depth += 1;
                            self.visit_expression(value);
                            self.function_depth -= 1;
                        }
                    }
                    ClassElement::StaticBlock(block) => {
                        self.function_depth += 1;
                        for stmt in &block.body {
                            self.visit_statement(stmt);
                        }
                        self.function_depth -= 1;
                    }
                    _ => {}
                }
            }
        }
        if let Some(name) = class_name {
            self.class_stack.push(name);
            walk::walk_class(self, it);
            self.class_stack.pop();
            return;
        }
        walk::walk_class(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        self.record_nested_reference(it.name.as_str());
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        let mut names = Vec::new();
        collect_assignment_target_names(&it.left, &mut names);
        for name in names {
            self.record_nested_reference(&name);
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_update_expression(&mut self, it: &UpdateExpression<'a>) {
        let mut names = Vec::new();
        collect_simple_assignment_target_names(&it.argument, &mut names);
        for name in names {
            self.record_nested_reference(&name);
        }
        walk::walk_update_expression(self, it);
    }
}

impl NestedRefCollector {
    fn record_nested_reference(&mut self, name: &str) {
        if self.function_depth == 0 {
            return;
        }
        if !self.top_level_bindings.contains(name) {
            return;
        }
        if self.local_exports.contains(name)
            && self
                .class_stack
                .last()
                .is_none_or(|class_name| class_name != name)
        {
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
            if cls.declare {
                return;
            }
            if let Some(id) = &cls.id {
                out.insert(id.name.as_str().to_string());
            }
        }
        Statement::VariableDeclaration(decl) => {
            if !decl.declare {
                push_variable_decl_names(decl, out);
            }
        }
        Statement::TSEnumDeclaration(decl) => {
            if !decl.declare {
                out.insert(decl.id.name.as_str().to_string());
            }
        }
        Statement::ExportNamedDeclaration(export) => match &export.declaration {
            Some(Declaration::FunctionDeclaration(func)) => push_function_name(func, out),
            Some(Declaration::ClassDeclaration(cls)) => {
                if cls.declare {
                    return;
                }
                if let Some(id) = &cls.id {
                    out.insert(id.name.as_str().to_string());
                }
            }
            Some(Declaration::VariableDeclaration(decl)) => {
                if !decl.declare {
                    push_variable_decl_names(decl, out);
                }
            }
            Some(Declaration::TSEnumDeclaration(decl)) => {
                if !decl.declare {
                    out.insert(decl.id.name.as_str().to_string());
                }
            }
            _ => {}
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                push_function_name(func, out)
            }
            ExportDefaultDeclarationKind::ClassDeclaration(cls) => {
                if cls.declare {
                    return;
                }
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
