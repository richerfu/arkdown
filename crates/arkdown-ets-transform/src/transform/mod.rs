mod build_fn;
mod component;
mod component_analysis;
mod component_collection;
pub mod component_meta;
mod declaration_emit;
pub(crate) mod decorator;
mod entry;
mod global_rewrite;
pub(crate) mod source_emit;
mod source_text;
mod syntax;
pub(crate) mod viewpu_emit;
mod viewpu_finalize;
mod viewv2;
pub(crate) use syntax::ArktsTransformSyntaxDetector;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustc_hash::FxHashSet;

use arkdown_core::ModuleSource;
use arkdown_path::{classify::is_inside_oh_modules, ArtifactPathSerializer, HvigorModulePath};
use oxc_allocator::{Allocator, CloneIn, Vec as OxcVec};
use oxc_ast::ast::{
    Class, ClassElement, Declaration, Decorator, ExportDefaultDeclarationKind, Expression,
    FormalParameterKind, Function, FunctionType, ImportOrExportKind, MethodDefinitionKind,
    MethodDefinitionType, Program, PropertyKey, Statement, StructElement, StructStatement,
    TSModuleDeclarationBody, TSType,
};
use oxc_ast::NONE;
use oxc_codegen::{CodegenOptions, CommentOptions, IndentChar};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, SPAN};

use crate::error::EtsTransformError;
use crate::lazy_import::LazyImportTransform;
use crate::source::TransformSourceSnapshot;
use crate::transform::build_fn::{BuildBodyInfo, BuildBodyPlan, CustomComponentUpdateProps};
use crate::transform::component::{
    BuilderMethodInfo, ComponentInfo, FieldExpression, ParamField, RegularField, RegularMember,
    StateField, StylesMethodInfo, ViewPuClassEmitter,
};
use crate::transform::component_analysis::{
    BuilderFunctionNameCollector, CustomComponentNameCollector, ExtendAttributeIndex,
    ImportedComponentNamesCollector, ImportedCustomComponentNamesCollector,
    LocalComponentUpdatePropsCollector, SimpleTypeContext, StructBuilderMethodNames,
    StructBuilderParamNames,
};
use crate::transform::component_collection::ComponentNameCollector;
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::declaration_emit::{ParamsInterfaceEmitter, TopLevelFunctionEmitter};
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::entry::{EntryOptions, EntryRegistrationBlock, EntryRegistrationInfo};
use crate::transform::source_emit::HvigorTypescriptPrinter;
use crate::transform::source_text::ViewpuOutputText;
use crate::transform::viewpu_emit::GeneratedViewpuName;
use crate::transform::viewpu_finalize::ViewpuProgramFinalizer;
use crate::transform::viewv2::ComponentV2Lowerer;
pub(crate) use component_analysis::ModuleExportIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckerMode {
    Oxc,
    Off,
}

#[derive(Debug, Clone)]
pub struct TransformOptions {
    pub bundle_name: Option<String>,
    pub module_name: Option<String>,
    pub project_root: Option<PathBuf>,
    pub module_roots: Vec<(String, PathBuf)>,
    pub component_metadata: Arc<ComponentMetadata>,
    pub sources: TransformSourceSnapshot,
    pub(crate) module_exports: ModuleExportIndex,
    pub emit_entry_registration: bool,
    /// Whether Stage model partial update is enabled (API >= 9).
    ///
    /// Defaults to `true`; set to `false` only for FA model / JSBUNDLE builds,
    /// which `validate_compile_mode()` in `arkdown-profile` rejects anyway.
    pub partial_update: bool,
    pub checker_mode: CheckerMode,
}

impl Default for TransformOptions {
    fn default() -> Self {
        Self {
            bundle_name: None,
            module_name: None,
            project_root: None,
            module_roots: Vec::new(),
            component_metadata: Arc::new(ComponentMetadata::default()),
            sources: TransformSourceSnapshot::default(),
            module_exports: ModuleExportIndex::default(),
            emit_entry_registration: true,
            partial_update: true,
            checker_mode: CheckerMode::Oxc,
        }
    }
}

impl TransformOptions {
    /// Whether Stage model partial update is enabled (API >= 9).
    pub fn supports_partial_update(&self) -> bool {
        self.partial_update
    }
}

pub struct ArkuiComponentNameCollector<'a> {
    content: &'a str,
    id: &'a str,
    options: Option<&'a TransformOptions>,
}

impl<'a> ArkuiComponentNameCollector<'a> {
    pub fn new(content: &'a str, id: &'a str) -> Self {
        Self {
            content,
            id,
            options: None,
        }
    }

    pub fn with_options(content: &'a str, id: &'a str, options: &'a TransformOptions) -> Self {
        Self {
            content,
            id,
            options: Some(options),
        }
    }

    /// Collect ArkUI component names from an ETS file using OXC's ArkUI AST nodes.
    ///
    /// Hvigor writes this information to `component_collection.json` during the
    /// widget CompileArkTS pass. The collection is based on parsed ArkUI
    /// component expressions rather than source-text matching.
    pub fn collect(&self) -> Result<Vec<String>, EtsTransformError> {
        let allocator = Allocator::default();
        let source_type = SourceType::from_path(self.id).unwrap_or_else(|_| SourceType::ets());
        let parsed = match self.options {
            Some(options) => {
                options
                    .component_metadata
                    .parse_ets(&allocator, self.content, source_type)
            }
            None => Parser::new(&allocator, self.content, source_type).parse(),
        };
        if !parsed.diagnostics.is_empty() {
            return Err(EtsTransformError::Fatal {
                file: self.id.to_string(),
                message: format!("ArkUI parse failed: {:?}", parsed.diagnostics),
            });
        }

        let mut custom_components = FxHashSet::default();
        let mut component_collector = CustomComponentNameCollector::new(&mut custom_components);
        for statement in &parsed.program.body {
            component_collector.collect_statement(statement);
        }
        if let Some(options) = self.options {
            ImportedCustomComponentNamesCollector::new(
                &parsed.program,
                self.id,
                options,
                &mut custom_components,
            )
            .collect();
        }
        Ok(ComponentNameCollector::new(custom_components).collect_program(&parsed.program))
    }
}

pub struct ArktsTransformer<'a> {
    source: &'a ModuleSource,
    options: ArktsTransformerOptions<'a>,
}

pub struct ArktsTransformOutput {
    pub code: String,
    pub source_map: Option<String>,
}

enum ArktsTransformerOptions<'a> {
    Owned(TransformOptions),
    Borrowed(&'a TransformOptions),
}

impl ArktsTransformerOptions<'_> {
    fn as_ref(&self) -> &TransformOptions {
        match self {
            Self::Owned(options) => options,
            Self::Borrowed(options) => options,
        }
    }
}

impl<'a> ArktsTransformer<'a> {
    /// Transform an ArkTS `.ets` source file into TypeScript.
    ///
    /// ArkUI parsing is handled by the workspace-pinned ohos-rs/oxc fork using
    /// `SourceType::ets()`, so component extraction uses real AST nodes instead
    /// of source text matching.
    pub fn new(source: &'a ModuleSource) -> Self {
        Self {
            source,
            options: ArktsTransformerOptions::Owned(TransformOptions::default()),
        }
    }

    pub fn with_options(source: &'a ModuleSource, options: &'a TransformOptions) -> Self {
        Self {
            source,
            options: ArktsTransformerOptions::Borrowed(options),
        }
    }

    pub fn transform(&self) -> Result<String, EtsTransformError> {
        self.transform_with_source_map().map(|output| output.code)
    }

    pub fn transform_with_source_map(&self) -> Result<ArktsTransformOutput, EtsTransformError> {
        let source = self.source;
        let options = self.options.as_ref();
        let content = source.content.as_str();

        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path(&source.id.id).unwrap_or_else(|_| SourceType::ets());
        let mut parsed = options
            .component_metadata
            .parse_ets(&allocator, content, source_type);
        if !parsed.diagnostics.is_empty() {
            return Err(EtsTransformError::Fatal {
                file: source.id.id.clone(),
                message: format!("ArkUI parse failed: {:?}", parsed.diagnostics),
            });
        }

        let syntax_detector = ArktsTransformSyntaxDetector::new();
        if !syntax_detector.program(&parsed.program) {
            return Ok(ArktsTransformOutput {
                code: source.content.clone(),
                source_map: None,
            });
        }

        if matches!(options.checker_mode, CheckerMode::Oxc) {
            let diagnostics =
                crate::checker::EtsChecker::new(&parsed.program, &source.id.id).check();
            for diag in &diagnostics {
                tracing::debug!(file = %source.id.id, diagnostic = ?diag, "ets checker diagnostic");
            }
            let demote_linter_errors = is_inside_oh_modules(Path::new(&source.id.id));
            if let Some(error) = diagnostics.iter().find(|diag| {
                if demote_linter_errors {
                    return false;
                }
                matches!(
                    diag,
                    EtsTransformError::Diagnostic {
                        severity: crate::error::DiagnosticSeverity::Error,
                        ..
                    } | EtsTransformError::Fatal { .. }
                )
            }) {
                return Err(error.clone());
            }
        }

        let simple_type_context =
            SimpleTypeContext::from_program(&parsed.program, &source.id.id, options);
        let hvigor_source_id = TransformSourcePath::new(&source.id.id).hvigor_source_path(options);
        let mut custom_components = FxHashSet::default();
        let mut custom_component_update_props = CustomComponentUpdateProps::default();
        let mut component_collector = CustomComponentNameCollector::new(&mut custom_components);
        let mut update_props_collector =
            LocalComponentUpdatePropsCollector::new(&mut custom_component_update_props);
        for statement in &parsed.program.body {
            component_collector.collect_statement(statement);
            update_props_collector.collect_statement(statement);
        }
        ImportedComponentNamesCollector::new(
            &parsed.program,
            &source.id.id,
            options,
            &mut custom_components,
            &mut custom_component_update_props,
        )
        .collect();
        let custom_dialog_components = custom_component_update_props
            .iter()
            .filter_map(|(name, info)| info.is_custom_dialog.then(|| name.clone()))
            .collect::<FxHashSet<_>>();
        let builder_functions =
            BuilderFunctionNameCollector::new(&parsed.program, &source.id.id, options).collect();
        let extend_attributes = Arc::new(ExtendAttributeIndex::from_program(&parsed.program));

        let statement_transformer = ArktsStatementTransformer::new(
            &allocator,
            content,
            &source.id.id,
            options,
            &simple_type_context,
            &custom_components,
            &custom_component_update_props,
            &builder_functions,
            &hvigor_source_id,
            extend_attributes,
        );
        let concurrent_directives_lowered =
            ConcurrentDirectiveLowerer::new(&allocator).program(&mut parsed.program);
        let mut entry_registrations = EntryRegistrationBlock::default();
        let mut reusable_v2_helper_emitted = false;
        let replacement_plans = parsed
            .program
            .body
            .iter()
            .map(|statement| {
                // V2 dispatch: @ComponentV2 structs are lowered through ComponentV2Lowerer,
                // not the V1 ViewPU path.
                let emit_reusable_v2_helper = !reusable_v2_helper_emitted
                    && statement_transformer.is_reusable_v2_component(statement);
                if let Some(replacement) =
                    statement_transformer.component_v2(statement, emit_reusable_v2_helper)?
                {
                    if options.emit_entry_registration {
                        if let Some((name, info)) =
                            statement_transformer.component_v2_entry_registration(statement)
                        {
                            entry_registrations.push(&name, Some(&info));
                        }
                    }
                    reusable_v2_helper_emitted |= emit_reusable_v2_helper;
                    Ok(Some(StatementReplacementPlan::AstStatements(replacement)))
                } else if let Some(component) = statement_transformer.component(statement)? {
                    if component.is_entry && options.emit_entry_registration {
                        entry_registrations
                            .push(&component.name, component.entry_registration.as_ref());
                    }
                    let statements = ViewPuClassEmitter::new(
                        &component,
                        options.component_metadata.runtime_names(),
                        &custom_dialog_components,
                    )
                    .statements(&allocator)
                    .map_err(|err| EtsTransformError::Fatal {
                        file: source.id.id.clone(),
                        message: format!("unsupported ArkUI build() DSL: {err}"),
                    })?;
                    Ok(Some(StatementReplacementPlan::AstStatements(statements)))
                } else {
                    statement_transformer.non_component_plan(statement)
                }
            })
            .collect::<Result<Vec<_>, EtsTransformError>>()?;

        if concurrent_directives_lowered || replacement_plans.iter().any(Option::is_some) {
            let entry_registration_statements = entry_registrations.statements(
                oxc_ast::AstBuilder::new(&allocator),
                &allocator,
                options.component_metadata.runtime_names(),
            );
            drop(entry_registrations);

            let mut body = OxcVec::new_in(&&allocator);
            let old_body = std::mem::replace(&mut parsed.program.body, OxcVec::new_in(&&allocator));
            for (statement, replacement_plan) in old_body.into_iter().zip(replacement_plans) {
                match replacement_plan {
                    Some(StatementReplacementPlan::AstStatements(statements)) => {
                        body.extend(statements);
                    }
                    Some(StatementReplacementPlan::LowerSendableClass) => {
                        let statements =
                            SendableClassLowerer::new(&allocator).statements(statement);
                        body.extend(statements);
                    }
                    None => body.push(statement),
                }
            }
            body.extend(entry_registration_statements);
            parsed.program.body = body;
            ViewpuProgramFinalizer::new(&allocator, &mut parsed.program).finalize();

            let generated = HvigorTypescriptPrinter::print_with_source_map(
                &parsed.program,
                CodegenOptions {
                    comments: CommentOptions::disabled(),
                    source_map_path: Some(source.id.id.clone().into()),
                    indent_char: IndentChar::Space,
                    indent_width: 4,
                    ..CodegenOptions::default()
                },
                content,
            );
            let out = ViewpuOutputText::new(generated.code).without_extra_trailing_blank_lines();
            Ok(ArktsTransformOutput {
                code: out,
                source_map: generated.source_map,
            })
        } else {
            Ok(ArktsTransformOutput {
                code: content.to_string(),
                source_map: None,
            })
        }
    }
}

struct ConcurrentDirectiveLowerer<'a> {
    allocator: &'a Allocator,
}

impl<'a> ConcurrentDirectiveLowerer<'a> {
    fn new(allocator: &'a Allocator) -> Self {
        Self { allocator }
    }

    fn program(&self, program: &mut Program<'a>) -> bool {
        let mut changed = false;
        for statement in &mut program.body {
            changed |= self.statement(statement);
        }
        changed
    }

    fn statement(&self, statement: &mut Statement<'a>) -> bool {
        match statement {
            Statement::FunctionDeclaration(function) => self.function(function),
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_mut() {
                Some(Declaration::FunctionDeclaration(function)) => self.function(function),
                Some(Declaration::TSModuleDeclaration(module)) => {
                    self.ts_module_body(&mut module.body)
                }
                _ => false,
            },
            Statement::TSModuleDeclaration(module) => self.ts_module_body(&mut module.body),
            _ => false,
        }
    }

    fn ts_module_body(&self, body: &mut Option<TSModuleDeclarationBody<'a>>) -> bool {
        match body {
            Some(TSModuleDeclarationBody::TSModuleDeclaration(module)) => {
                self.ts_module_body(&mut module.body)
            }
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                let mut changed = false;
                for statement in &mut block.body {
                    changed |= self.statement(statement);
                }
                changed
            }
            None => false,
        }
    }

    fn function(&self, function: &mut Function<'a>) -> bool {
        if !function.decorators.iter().any(|decorator| {
            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CONCURRENT
        }) {
            return false;
        }
        function.decorators.clear();
        if let Some(body) = function.body.as_mut() {
            let ast = oxc_ast::AstBuilder::new(self.allocator);
            let directive = ast.directive(
                SPAN,
                ast.string_literal(SPAN, ast.str(ConcurrentFunctionEmitter::DIRECTIVE), None),
                ast.str(ConcurrentFunctionEmitter::DIRECTIVE),
            );
            body.directives.insert(0, directive);
        }
        true
    }
}

enum StatementReplacementPlan<'a> {
    AstStatements(OxcVec<'a, Statement<'a>>),
    LowerSendableClass,
}

struct ArktsStatementTransformer<'a, 'b> {
    allocator: &'b Allocator,
    source_text: Arc<str>,
    id: &'a str,
    options: &'a TransformOptions,
    simple_type_context: &'a SimpleTypeContext,
    custom_components: &'a FxHashSet<String>,
    custom_component_update_props: &'a CustomComponentUpdateProps,
    builder_functions: &'a FxHashSet<String>,
    extend_attributes: Arc<ExtendAttributeIndex>,
    hvigor_source_id: Arc<str>,
    module_name: Option<Arc<str>>,
}

impl<'a, 'b> ArktsStatementTransformer<'a, 'b> {
    fn new(
        allocator: &'b Allocator,
        source_text: &'a str,
        id: &'a str,
        options: &'a TransformOptions,
        simple_type_context: &'a SimpleTypeContext,
        custom_components: &'a FxHashSet<String>,
        custom_component_update_props: &'a CustomComponentUpdateProps,
        builder_functions: &'a FxHashSet<String>,
        hvigor_source_id: &'a str,
        extend_attributes: Arc<ExtendAttributeIndex>,
    ) -> Self {
        Self {
            allocator,
            source_text: Arc::from(source_text),
            id,
            options,
            simple_type_context,
            custom_components,
            custom_component_update_props,
            builder_functions,
            extend_attributes,
            hvigor_source_id: Arc::from(hvigor_source_id),
            module_name: options.module_name.as_deref().map(Arc::from),
        }
    }

    fn component_v2(
        &self,
        statement: &Statement<'_>,
        emit_reusable_v2_helper: bool,
    ) -> Result<Option<OxcVec<'b, Statement<'b>>>, EtsTransformError> {
        let (struct_stmt, is_exported, is_default) = match statement {
            Statement::StructStatement(s) => (s.as_ref(), false, false),
            Statement::ExportNamedDeclaration(e) => match e.declaration.as_ref() {
                Some(Declaration::StructStatement(s)) => (s.as_ref(), true, false),
                _ => return Ok(None),
            },
            Statement::ExportDefaultDeclaration(e) => match &e.declaration {
                ExportDefaultDeclarationKind::StructStatement(s) => (s.as_ref(), true, true),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };

        if !ArkuiDecoratorRules::is_component_v2_struct(struct_stmt) {
            return Ok(None);
        }

        match ComponentV2Lowerer::new(
            struct_stmt,
            self.allocator,
            self.options.component_metadata.runtime_names(),
            self.source_text.as_ref(),
            self.hvigor_source_id.as_ref(),
            self.custom_components,
            self.builder_functions,
            self.custom_component_update_props,
            &self.options.component_metadata,
            Arc::clone(&self.extend_attributes),
            self.options.module_name.as_deref(),
            is_exported,
            is_default,
            struct_stmt.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENTRY
            }),
            emit_reusable_v2_helper,
        )
        .lower()
        {
            Ok(replacement) => Ok(Some(replacement)),
            Err(err) => Err(EtsTransformError::Fatal {
                file: self.id.to_string(),
                message: format!("unsupported ArkUI @ComponentV2 DSL: {err}"),
            }),
        }
    }

    fn is_reusable_v2_component(&self, statement: &Statement<'_>) -> bool {
        let struct_stmt = match statement {
            Statement::StructStatement(s) => s.as_ref(),
            Statement::ExportNamedDeclaration(e) => match e.declaration.as_ref() {
                Some(Declaration::StructStatement(s)) => s.as_ref(),
                _ => return false,
            },
            Statement::ExportDefaultDeclaration(e) => match &e.declaration {
                ExportDefaultDeclarationKind::StructStatement(s) => s.as_ref(),
                _ => return false,
            },
            _ => return false,
        };
        ArkuiDecoratorRules::is_component_v2_struct(struct_stmt)
            && struct_stmt.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE_V2
            })
    }

    fn component_v2_entry_registration<'s>(
        &self,
        statement: &'s Statement<'s>,
    ) -> Option<(String, EntryRegistrationInfo<'s>)> {
        let struct_stmt = match statement {
            Statement::StructStatement(s) => s.as_ref(),
            Statement::ExportNamedDeclaration(e) => match e.declaration.as_ref() {
                Some(Declaration::StructStatement(s)) => s.as_ref(),
                _ => return None,
            },
            Statement::ExportDefaultDeclaration(e) => match &e.declaration {
                ExportDefaultDeclarationKind::StructStatement(s) => s.as_ref(),
                _ => return None,
            },
            _ => return None,
        };
        if !ArkuiDecoratorRules::is_component_v2_struct(struct_stmt) {
            return None;
        }
        let entry_decorator = struct_stmt
            .decorators
            .iter()
            .find(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENTRY)?;
        let storage = ArkuiDecorator::new(entry_decorator).arg_expression();
        Some((
            struct_stmt.id.name.to_string(),
            EntryRegistrationFactory::new(self.hvigor_source_id.as_ref(), self.options)
                .with_entry_argument(storage),
        ))
    }

    fn component<'s>(
        &self,
        statement: &'s Statement<'s>,
    ) -> Result<Option<ComponentInfo<'s>>, EtsTransformError> {
        match statement {
            Statement::StructStatement(statement) => {
                self.component_from_struct(statement, false, false)
            }
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                Some(Declaration::StructStatement(statement)) => {
                    self.component_from_struct(statement, true, false)
                }
                _ => Ok(None),
            },
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::StructStatement(statement) => {
                    self.component_from_struct(statement, true, true)
                }
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    fn non_component_plan(
        &self,
        statement: &Statement<'_>,
    ) -> Result<Option<StatementReplacementPlan<'b>>, EtsTransformError> {
        let replacement = match statement {
            Statement::AnnotationDeclaration(annotation) => {
                Some(StatementReplacementPlan::AstStatements(
                    oxc_ast::AstBuilder::new(self.allocator).vec1(
                        ParamsInterfaceEmitter::new(annotation).statement(self.allocator, false),
                    ),
                ))
            }
            Statement::LazyImportDeclaration(declaration) => {
                Some(StatementReplacementPlan::AstStatements(
                    oxc_ast::AstBuilder::new(self.allocator).vec1(
                        LazyImportTransform::import_statement(declaration, self.allocator),
                    ),
                ))
            }
            Statement::ClassDeclaration(_)
                if SendableClassLowerer::statement_needs_lowering(statement) =>
            {
                Some(StatementReplacementPlan::LowerSendableClass)
            }
            Statement::FunctionDeclaration(function) => self
                .decorated_function_statement(function, false, &[])?
                .map(|statement| {
                    StatementReplacementPlan::AstStatements(
                        oxc_ast::AstBuilder::new(self.allocator).vec1(statement),
                    )
                }),
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                Some(Declaration::FunctionDeclaration(function)) => self
                    .decorated_function_statement(function, true, &export.decorators)?
                    .map(|statement| {
                        StatementReplacementPlan::AstStatements(
                            oxc_ast::AstBuilder::new(self.allocator).vec1(statement),
                        )
                    }),
                Some(Declaration::AnnotationDeclaration(annotation)) => {
                    Some(StatementReplacementPlan::AstStatements(
                        oxc_ast::AstBuilder::new(self.allocator).vec1(
                            ParamsInterfaceEmitter::new(annotation).statement(self.allocator, true),
                        ),
                    ))
                }
                Some(Declaration::ClassDeclaration(_))
                    if SendableClassLowerer::statement_needs_lowering(statement) =>
                {
                    Some(StatementReplacementPlan::LowerSendableClass)
                }
                _ => None,
            },
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::ClassDeclaration(_)
                    if SendableClassLowerer::statement_needs_lowering(statement) =>
                {
                    Some(StatementReplacementPlan::LowerSendableClass)
                }
                _ => None,
            },
            _ => None,
        };
        Ok(replacement)
    }

    fn decorated_function_statement(
        &self,
        function: &Function<'_>,
        exported: bool,
        extra_decorators: &[Decorator<'_>],
    ) -> Result<Option<Statement<'b>>, EtsTransformError> {
        let decorators = function.decorators.iter().chain(extra_decorators.iter());
        let decorator_kind = decorators
            .clone()
            .map(|decorator| ArkuiDecorator::new(decorator).name())
            .find(|name| ArkuiDecoratorName::is_top_level_function(name))
            .map(str::to_string);
        let Some(decorator_kind) = decorator_kind else {
            return Ok(None);
        };
        let Some(function_id) = function.id.as_ref() else {
            return Ok(None);
        };
        let name = function_id.name.as_str();
        let append_parent = ArkuiDecoratorName::appends_parent_parameter(decorator_kind.as_str());
        let return_annotation = function.return_type.as_deref();
        let statement = match decorator_kind.as_str() {
            ArkuiDecoratorName::CONCURRENT => {
                ConcurrentFunctionEmitter::new(self.allocator).statement(function, exported)
            }
            ArkuiDecoratorName::STYLES => TopLevelFunctionEmitter::decorated_styles_statement(
                self.allocator,
                exported,
                name,
                &function.params,
                return_annotation,
                function.body.as_deref(),
            ),
            decorator_name if ArkuiDecoratorName::is_extend_function(decorator_name) => {
                let component = function
                    .decorators
                    .iter()
                    .chain(extra_decorators.iter())
                    .find(|decorator| ArkuiDecorator::new(decorator).name() == decorator_kind)
                    .and_then(|decorator| ArkuiDecorator::new(decorator).identifier_arg())
                    .unwrap_or_else(|| "View".to_string());
                TopLevelFunctionEmitter::extend_with_body_statement(
                    self.allocator,
                    exported,
                    &component,
                    name,
                    &function.params,
                    function.body.as_deref(),
                )
            }
            _ => {
                let raw_object_fields = FxHashSet::default();
                let body = BuildBodyPlan::builder_with_shared_inputs(
                    function.body.as_deref(),
                    Arc::clone(&self.source_text),
                    Arc::clone(&self.hvigor_source_id),
                    self.custom_components,
                    self.builder_functions,
                    &FxHashSet::default(),
                    self.custom_component_update_props,
                    Arc::clone(&self.options.component_metadata),
                    &raw_object_fields,
                    None,
                    GeneratedViewpuName::VIEWPU_BASE,
                )
                .with_shared_extend_attributes(Arc::clone(&self.extend_attributes));
                TopLevelFunctionEmitter::decorated_with_params_statement(
                    self.allocator,
                    exported,
                    name,
                    &function.params,
                    append_parent,
                    return_annotation,
                    &BuildBodyInfo::Plan(body),
                )
                .map_err(|err| EtsTransformError::Fatal {
                    file: self.id.to_string(),
                    message: format!("unsupported ArkUI builder DSL: {err:?}"),
                })?
            }
        };

        Ok(Some(statement))
    }

    fn component_from_struct<'s>(
        &self,
        statement: &'s StructStatement<'s>,
        is_exported: bool,
        is_default_export: bool,
    ) -> Result<Option<ComponentInfo<'s>>, EtsTransformError> {
        if !ArkuiDecoratorRules::is_component_struct(statement) {
            return Ok(None);
        }

        let is_entry = statement
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENTRY);
        let entry_span = statement
            .decorators
            .iter()
            .find(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENTRY)
            .map_or(SPAN, GetSpan::span);
        let is_reusable = statement
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::REUSABLE);
        let mut state_fields = Vec::new();
        let mut initial_render_body = BuildBodyInfo::default();
        let mut initial_render_method_span = SPAN;
        let mut builder_methods: Vec<BuilderMethodInfo> = Vec::new();
        let builder_method_names = StructBuilderMethodNames::new(statement).collect();
        let builder_param_names = StructBuilderParamNames::new(statement).collect();
        let mut visible_builder_methods = self.builder_functions.clone();
        visible_builder_methods.extend(builder_method_names.iter().cloned());
        visible_builder_methods.extend(builder_param_names.iter().cloned());
        let raw_object_fields =
            ComponentRawObjectFields::new(statement, self.simple_type_context).collect();
        let mut styles_methods: Vec<StylesMethodInfo> = Vec::new();
        let mut regular_members: Vec<RegularMember<'_>> = Vec::new();
        let mut params_fields: Vec<ParamField> = Vec::new();
        let mut property_order: Vec<String> = Vec::new();
        let mut regular_fields: Vec<RegularField> = Vec::new();

        for element in &statement.body.body {
            match element {
                StructElement::PropertyDefinition(property) => {
                    if property.r#static {
                        // SDK ets-loader parity:
                        // process_component_class.js::processMembers keeps static struct
                        // properties as class members with `d.push(e)` and does not route
                        // them through `createVariableInitStatement`, params, or constructor
                        // initialization.
                        regular_members.push(RegularMember::Property(property));
                        continue;
                    }

                    if let Some(name) = property.key.static_name() {
                        let name = name.into_owned();
                        property_order.push(name.clone());
                        params_fields.push(ParamField {
                            name,
                            source_span: property.span,
                            name_span: property.key.span(),
                            type_annotation_ast: property.type_annotation.as_deref(),
                        });
                    }

                    let decorator_names: Vec<&str> = property
                        .decorators
                        .iter()
                        .map(|decorator| ArkuiDecorator::new(decorator).name())
                        .collect();
                    let Some(decorator) = ArkuiDecoratorRules::primary_state(&decorator_names)
                    else {
                        if let Some(name) = property.key.static_name() {
                            let initializer = property
                                .value
                                .as_ref()
                                .map(FieldExpression::from_expression);
                            regular_fields.push(RegularField {
                                name: name.into_owned(),
                                source_span: property.span,
                                name_span: property.key.span(),
                                type_annotation_ast: property.type_annotation.as_deref(),
                                initializer,
                                optional: property.optional,
                            });
                        } else {
                            regular_members.push(RegularMember::Property(property));
                        }
                        continue;
                    };
                    let Some(name) = property.key.static_name() else {
                        continue;
                    };
                    let type_node = property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| &annotation.type_annotation);
                    let initializer = property
                        .value
                        .as_ref()
                        .map(FieldExpression::from_expression);
                    let watch_handler = property
                        .decorators
                        .iter()
                        .find(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::WATCH)
                        .and_then(|decorator| ArkuiDecorator::new(decorator).string_arg());
                    let alias = property
                        .decorators
                        .iter()
                        .find(|d| {
                            ArkuiDecoratorName::is_provider_alias(ArkuiDecorator::new(d).name())
                        })
                        .and_then(|decorator| ArkuiDecorator::new(decorator).string_arg());
                    let decorator_arg = property
                        .decorators
                        .iter()
                        .find(|d| ArkuiDecorator::new(d).name() == decorator)
                        .and_then(|decorator| ArkuiDecorator::new(decorator).arg_expression())
                        .map(FieldExpression::from_expression);
                    state_fields.push(StateField {
                        name: name.into_owned(),
                        source_span: property.span,
                        name_span: property.key.span(),
                        type_annotation_ast: property.type_annotation.as_deref(),
                        optional: property.optional,
                        decorator: decorator.to_string(),
                        initializer,
                        watch_handler,
                        alias,
                        decorator_arg,
                        is_simple_type: type_node.is_none_or(|ty| {
                            self.simple_type_context.is_simple_type_annotation(ty)
                        }),
                    });
                }
                StructElement::MethodDefinition(method)
                    if method
                        .key
                        .static_name()
                        .is_some_and(|name| name == GeneratedViewpuName::BUILD) =>
                {
                    let build_method_span = method.span;
                    let plan = BuildBodyPlan::component_with_shared_inputs(
                        method.value.body.as_deref(),
                        Arc::clone(&self.source_text),
                        Arc::clone(&self.hvigor_source_id),
                        self.custom_components,
                        &visible_builder_methods,
                        &builder_param_names,
                        self.custom_component_update_props,
                        Arc::clone(&self.options.component_metadata),
                        &raw_object_fields,
                        self.module_name.as_ref().map(Arc::clone),
                        is_reusable,
                        GeneratedViewpuName::VIEWPU_BASE,
                    )
                    .with_shared_extend_attributes(Arc::clone(&self.extend_attributes));
                    initial_render_body = BuildBodyInfo::Plan(plan);
                    // Preserve the original build method range on the generated
                    // initialRender method for TypeScript-printer source maps.
                    // There is only one valid build method per component.
                    initial_render_method_span = build_method_span;
                }
                StructElement::MethodDefinition(method)
                    if method
                        .decorators
                        .iter()
                        .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::BUILDER) =>
                {
                    let Some(name) = method.key.static_name() else {
                        continue;
                    };
                    let body = BuildBodyPlan::builder_with_shared_inputs(
                        method.value.body.as_deref(),
                        Arc::clone(&self.source_text),
                        Arc::clone(&self.hvigor_source_id),
                        self.custom_components,
                        &visible_builder_methods,
                        &builder_param_names,
                        self.custom_component_update_props,
                        Arc::clone(&self.options.component_metadata),
                        &raw_object_fields,
                        self.module_name.as_ref().map(Arc::clone),
                        GeneratedViewpuName::VIEWPU_BASE,
                    )
                    .with_shared_extend_attributes(Arc::clone(&self.extend_attributes));
                    builder_methods.push(BuilderMethodInfo {
                        name: name.into_owned(),
                        params: &method.value.params,
                        return_annotation: method.value.return_type.as_deref(),
                        body: BuildBodyInfo::Plan(body),
                    });
                }
                StructElement::MethodDefinition(method)
                    if method
                        .decorators
                        .iter()
                        .any(|d| ArkuiDecorator::new(d).name() == ArkuiDecoratorName::STYLES) =>
                {
                    let Some(name) = method.key.static_name() else {
                        continue;
                    };
                    styles_methods.push(StylesMethodInfo {
                        name: name.into_owned(),
                        params: &method.value.params,
                        body: method.value.body.as_deref(),
                    });
                }
                StructElement::MethodDefinition(method) => {
                    if method.kind != MethodDefinitionKind::Constructor
                        && method
                            .key
                            .static_name()
                            .is_none_or(|name| name != GeneratedViewpuName::CONSTRUCTOR)
                    {
                        regular_members.push(RegularMember::Method(method));
                    }
                }
                StructElement::StaticBlock(block) => {
                    regular_members.push(RegularMember::StaticBlock(block));
                }
                StructElement::TSIndexSignature(signature) => {
                    regular_members.push(RegularMember::TSIndexSignature(signature));
                }
                StructElement::AccessorProperty(property) => {
                    regular_members.push(RegularMember::AccessorProperty(property));
                }
            }
        }

        Ok(Some(ComponentInfo {
            partial_update: self.options.supports_partial_update(),
            name: statement.id.name.to_string(),
            name_span: statement.id.span,
            entry_span,
            build_method_span: initial_render_method_span,
            is_entry,
            is_exported,
            is_default_export,
            is_reusable,
            is_custom_dialog: statement.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::CUSTOM_DIALOG
            }),
            state_fields,
            params_fields,
            property_order,
            regular_fields,
            regular_members,
            initial_render_body,
            entry_registration: is_entry.then(|| {
                let storage = statement
                    .decorators
                    .iter()
                    .find(|decorator| {
                        ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::ENTRY
                    })
                    .and_then(|decorator| ArkuiDecorator::new(decorator).arg_expression());
                EntryRegistrationFactory::new(self.hvigor_source_id.as_ref(), self.options)
                    .with_entry_argument(storage)
            }),
            builder_methods,
            styles_methods,
        }))
    }
}

struct ConcurrentFunctionEmitter<'a> {
    allocator: &'a Allocator,
}

impl<'a> ConcurrentFunctionEmitter<'a> {
    const DIRECTIVE: &'static str = "use concurrent";

    fn new(allocator: &'a Allocator) -> Self {
        Self { allocator }
    }

    fn statement(&self, function: &Function<'_>, exported: bool) -> Statement<'a> {
        let ast = oxc_ast::AstBuilder::new(self.allocator);
        let mut function = function.clone_in(self.allocator);
        function.decorators.clear();
        if let Some(body) = function.body.as_mut() {
            let directive = ast.directive(
                SPAN,
                ast.string_literal(SPAN, ast.str(Self::DIRECTIVE), None),
                ast.str(Self::DIRECTIVE),
            );
            body.directives.insert(0, directive);
        }
        let function = ast.alloc(function);
        if !exported {
            return Statement::FunctionDeclaration(function);
        }
        Statement::ExportNamedDeclaration(ast.alloc_export_named_declaration(
            SPAN,
            Some(Declaration::FunctionDeclaration(function)),
            ast.vec(),
            None,
            ImportOrExportKind::Value,
            NONE,
        ))
    }
}

/// Implements SDK `process_sendable.js::processSendableClass`.
///
/// `process_ui_syntax.js` leaves every ordinary class decorator intact and
/// dispatches only classes carrying `@Sendable` to this transform. Keeping
/// that distinction is important: ArkUI and application decorators are part
/// of the CompileArkTS input consumed by es2abc, not TypeScript decorators for
/// arkdown to erase or lower.
struct SendableClassLowerer<'a> {
    allocator: &'a Allocator,
    ast: oxc_ast::AstBuilder<'a>,
}

impl<'a> SendableClassLowerer<'a> {
    const DIRECTIVE: &'static str = "use sendable";
    const SUPER_ARGS: &'static str = "args";

    fn new(allocator: &'a Allocator) -> Self {
        Self {
            allocator,
            ast: oxc_ast::AstBuilder::new(allocator),
        }
    }

    fn statement_needs_lowering(statement: &Statement<'_>) -> bool {
        Self::statement_has_class_decorator(statement, ArkuiDecoratorName::SENDABLE)
    }

    fn statements(&self, mut statement: Statement<'a>) -> OxcVec<'a, Statement<'a>> {
        match &mut statement {
            Statement::ClassDeclaration(class) => self.class(class),
            Statement::ExportNamedDeclaration(export) => {
                Self::remove_sendable_decorator(&mut export.decorators);
                if let Some(Declaration::ClassDeclaration(class)) = &mut export.declaration {
                    self.class(class);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) =
                    &mut export.declaration
                {
                    self.class(class);
                }
            }
            _ => {}
        }
        self.ast.vec1(statement)
    }

    fn statement_has_class_decorator(statement: &Statement<'_>, name: &str) -> bool {
        match statement {
            Statement::ClassDeclaration(class) => Self::class_has_decorator(class, name),
            Statement::ExportNamedDeclaration(export) => {
                export
                    .decorators
                    .iter()
                    .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
                    || matches!(
                        export.declaration.as_ref(),
                        Some(Declaration::ClassDeclaration(class)) if Self::class_has_decorator(class, name)
                    )
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &export.declaration {
                    Self::class_has_decorator(class, name)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn class_has_decorator(class: &Class<'_>, name: &str) -> bool {
        class
            .decorators
            .iter()
            .any(|decorator| ArkuiDecorator::new(decorator).name() == name)
    }

    fn class(&self, class: &mut Class<'a>) {
        Self::remove_sendable_decorator(&mut class.decorators);
        let mut has_constructor = false;
        for element in &mut class.body.body {
            match element {
                ClassElement::PropertyDefinition(property) if property.optional => {
                    self.optional_property(property);
                }
                ClassElement::MethodDefinition(method)
                    if method.kind == MethodDefinitionKind::Constructor =>
                {
                    has_constructor = true;
                    if let Some(body) = method.value.body.as_mut() {
                        body.directives.insert(0, self.directive());
                    }
                }
                _ => {}
            }
        }
        if !has_constructor {
            let constructor = self.constructor(class.super_class.is_some());
            class.body.body.insert(0, constructor);
        }
    }

    fn optional_property(&self, property: &mut oxc_ast::ast::PropertyDefinition<'a>) {
        property.optional = false;
        if let Some(annotation) = property.type_annotation.as_mut() {
            let original = annotation.type_annotation.clone_in(self.allocator);
            annotation.type_annotation = match original {
                TSType::TSUnionType(mut union) => {
                    if !union
                        .types
                        .iter()
                        .any(|ty| matches!(ty, TSType::TSUndefinedKeyword(_)))
                    {
                        union.types.push(self.ast.ts_type_undefined_keyword(SPAN));
                    }
                    TSType::TSUnionType(union)
                }
                ty => self.ast.ts_type_union_type(
                    SPAN,
                    self.ast
                        .vec_from_iter([ty, self.ast.ts_type_undefined_keyword(SPAN)]),
                ),
            };
        }
        if property.value.is_none() {
            property.value = Some(
                self.ast
                    .expression_identifier(SPAN, self.ast.str("undefined")),
            );
        }
    }

    fn constructor(&self, has_super_class: bool) -> ClassElement<'a> {
        let rest = has_super_class.then(|| {
            self.ast.alloc_formal_parameter_rest(
                SPAN,
                self.ast.vec(),
                self.ast.binding_rest_element(
                    SPAN,
                    self.ast
                        .binding_pattern_binding_identifier(SPAN, self.ast.str(Self::SUPER_ARGS)),
                ),
                NONE,
            )
        });
        let params = self.ast.alloc_formal_parameters(
            SPAN,
            FormalParameterKind::FormalParameter,
            self.ast.vec(),
            rest,
        );
        let mut statements = self.ast.vec();
        if has_super_class {
            let arguments = self.ast.vec1(
                self.ast.argument_spread_element(
                    SPAN,
                    self.ast
                        .expression_identifier(SPAN, self.ast.str(Self::SUPER_ARGS)),
                ),
            );
            let call = self.ast.expression_call(
                SPAN,
                self.ast.expression_super(SPAN),
                NONE,
                arguments,
                false,
            );
            statements.push(self.ast.statement_expression(SPAN, call));
        }
        let body = self
            .ast
            .function_body(SPAN, self.ast.vec1(self.directive()), statements);
        let function = self.ast.alloc_function(
            SPAN,
            FunctionType::FunctionExpression,
            None,
            false,
            false,
            false,
            NONE,
            NONE,
            params,
            NONE,
            Some(body),
        );
        self.ast.class_element_method_definition(
            SPAN,
            MethodDefinitionType::MethodDefinition,
            self.ast.vec(),
            PropertyKey::StaticIdentifier(
                self.ast
                    .alloc_identifier_name(SPAN, self.ast.str("constructor")),
            ),
            function,
            MethodDefinitionKind::Constructor,
            false,
            false,
            false,
            false,
            None,
        )
    }

    fn directive(&self) -> oxc_ast::ast::Directive<'a> {
        self.ast.directive(
            SPAN,
            self.ast
                .string_literal(SPAN, self.ast.str(Self::DIRECTIVE), None),
            self.ast.str(Self::DIRECTIVE),
        )
    }

    fn remove_sendable_decorator(decorators: &mut OxcVec<'a, Decorator<'a>>) {
        decorators.retain(|decorator| {
            ArkuiDecorator::new(decorator).name() != ArkuiDecoratorName::SENDABLE
        });
    }
}

struct ArkuiDecoratorRules;

impl ArkuiDecoratorRules {
    /// Returns true when the struct is a V1 component: `@Component` or `@CustomDialog`.
    ///
    /// `@ComponentV2` is intentionally excluded and lowered through `ComponentV2Lowerer`.
    fn is_component_struct(statement: &StructStatement<'_>) -> bool {
        statement.decorators.iter().any(|decorator| {
            ArkuiDecoratorName::is_component_struct(ArkuiDecorator::new(decorator).name())
        })
    }

    fn is_component_v2_struct(statement: &StructStatement<'_>) -> bool {
        statement.decorators.iter().any(|decorator| {
            ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::COMPONENT_V2
        })
    }

    fn primary_state(names: &[&str]) -> Option<&'static str> {
        ArkuiDecoratorName::primary_state(names)
    }
}

struct ComponentRawObjectFields<'a, 'b> {
    statement: &'a StructStatement<'a>,
    simple_type_context: &'b SimpleTypeContext,
}

impl<'a, 'b> ComponentRawObjectFields<'a, 'b> {
    fn new(statement: &'a StructStatement<'a>, simple_type_context: &'b SimpleTypeContext) -> Self {
        Self {
            statement,
            simple_type_context,
        }
    }

    fn collect(&self) -> FxHashSet<String> {
        let mut names = FxHashSet::default();
        for element in &self.statement.body.body {
            let StructElement::PropertyDefinition(property) = element else {
                continue;
            };
            let decorator_names: Vec<&str> = property
                .decorators
                .iter()
                .map(|decorator| ArkuiDecorator::new(decorator).name())
                .collect();
            let Some(decorator) = ArkuiDecoratorRules::primary_state(&decorator_names) else {
                continue;
            };
            if decorator == ArkuiDecoratorName::BUILDER_PARAM {
                continue;
            }
            let Some(name) = property.key.static_name() else {
                continue;
            };
            let Some(type_node) = property
                .type_annotation
                .as_ref()
                .map(|annotation| &annotation.type_annotation)
            else {
                continue;
            };
            if !self
                .simple_type_context
                .is_simple_type_annotation(type_node)
            {
                names.insert(name.into_owned());
            }
        }
        names
    }
}

struct EntryRegistrationFactory<'a> {
    id: &'a str,
    options: &'a TransformOptions,
}

impl<'a> EntryRegistrationFactory<'a> {
    fn new(id: &'a str, options: &'a TransformOptions) -> Self {
        Self { id, options }
    }

    fn with_entry_argument<'b>(
        &self,
        argument: Option<&'b Expression<'b>>,
    ) -> EntryRegistrationInfo<'b> {
        let source_path = TransformSourcePath::new(self.id);
        let module_name = self
            .options
            .module_name
            .clone()
            .or_else(|| source_path.module_name())
            .unwrap_or_else(|| "entry".to_string());
        let page_path = source_path.entry_page_path();
        let page_full_path = source_path.extensionless_path("ets");

        EntryRegistrationInfo {
            bundle_name: self.options.bundle_name.clone().unwrap_or_default(),
            module_name,
            page_path,
            page_full_path,
            integrated_hsp: "false".to_string(),
            module_type: "followWithHap".to_string(),
            options: EntryOptions::from_argument(argument),
        }
    }
}

struct TransformSourcePath<'a> {
    id: &'a str,
}

impl<'a> TransformSourcePath<'a> {
    fn new(id: &'a str) -> Self {
        Self { id }
    }

    fn module_name(&self) -> Option<String> {
        HvigorModulePath::new(self.id).module_name()
    }

    fn entry_page_path(&self) -> String {
        HvigorModulePath::new(self.id).entry_page_path()
    }

    fn extensionless_path(&self, extension: &str) -> String {
        HvigorModulePath::new(self.id).extensionless_path(extension)
    }

    fn hvigor_source_path(&self, options: &TransformOptions) -> String {
        if let Some(path) = HvigorModulePath::new(self.id).hvigor_segment_path() {
            return path;
        }

        let source_path = Path::new(self.id);
        if let Some(project_root) = &options.project_root {
            if let Ok(relative) = source_path.strip_prefix(project_root) {
                return ArtifactPathSerializer::rel_to_slash(relative);
            }
        }

        self.id.to_string()
    }
}

#[cfg(test)]
mod tests;
