//! TypeScript source helpers for CompileArkTS cache parity.
//!
//! hvigor ESMODULE/CompileArkTS writes transformed TypeScript cache files and
//! passes them to `es2abc` through `filesInfo.txt`/`modules.cache`. Production
//! code keeps that TypeScript shape except for files carrying TypeScript's
//! `@ts-ignore`/`@ts-nocheck` pragmas. SDK `process_kit_import.js` records those
//! files and `module_mode.js` emits JavaScript for them. The OXC emitter below
//! implements that explicit production boundary.

use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use arkdown_core::ArkTscTarget;
use oxc_allocator::{Allocator, ArenaVec, CloneIn, GetAllocator};
use oxc_ast::ast::{
    Argument, AssignmentTarget, ChainElement, ComputedMemberExpression, Declaration,
    ExportAllDeclaration, ExportDefaultDeclarationKind, ExportFromDeclaration,
    ExportNamedDeclaration, Expression, IdentifierReference, ImportDeclaration,
    ImportDeclarationSpecifier, MethodDefinition, MethodDefinitionKind, Program, Statement,
    StaticMemberExpression, TSNamespaceDeclaration, TSNamespaceDeclarationBody,
    VariableDeclaration, VariableDeclarationKind,
};
use oxc_ast::builder::AstBuilder;
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{SourceType, SPAN};
use oxc_str::{Ident, Str};
use oxc_syntax::{
    number::NumberBase,
    operator::{AssignmentOperator, LogicalOperator, UnaryOperator},
    symbol::SymbolId,
};
use oxc_transformer::{TransformOptions, Transformer};

use crate::source::{
    CompileArktsSourceType, SourceClassifier, TransformSourceFile, TransformSourceSnapshot,
};
use crate::source_map::TypeScriptJavaScriptSourceMapBuilder;
use crate::ModuleExportNameText;

#[derive(Debug, Clone, PartialEq)]
enum EnumValue {
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, Default)]
struct ConstEnumExports {
    enums: FxHashMap<String, ImportedEnum>,
    namespaces: FxHashMap<String, FxHashMap<String, ImportedEnum>>,
}

#[derive(Debug, Clone)]
enum ConstEnumImport {
    Enum(ImportedEnum),
    Namespace(FxHashMap<String, ImportedEnum>),
}

#[derive(Debug, Clone)]
struct ImportedEnum {
    members: FxHashMap<String, EnumValue>,
    inline_everywhere: bool,
}

impl ImportedEnum {
    fn const_enum(members: FxHashMap<String, EnumValue>) -> Self {
        Self {
            members,
            inline_everywhere: true,
        }
    }

    fn ambient_enum(members: FxHashMap<String, EnumValue>) -> Self {
        Self {
            members,
            inline_everywhere: false,
        }
    }
}

pub(crate) struct TypeScriptJavascriptEmitter<'a> {
    source: &'a str,
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
    const_enum_exports: Option<&'a ConstEnumExportCache>,
    hvigor_emit_wrapper: bool,
    tsc_target: ArkTscTarget,
}

pub(crate) struct TypeScriptJavascriptEmitOutput {
    pub(crate) code: String,
    pub(crate) source_map: Option<String>,
}

impl<'a> TypeScriptJavascriptEmitter<'a> {
    pub(crate) fn new(source: &'a str, id: &'a str, sources: &'a TransformSourceSnapshot) -> Self {
        Self {
            source,
            id,
            sources,
            const_enum_exports: None,
            hvigor_emit_wrapper: false,
            tsc_target: ArkTscTarget::default(),
        }
    }

    pub(crate) fn with_const_enum_export_cache(mut self, cache: &'a ConstEnumExportCache) -> Self {
        self.const_enum_exports = Some(cache);
        self
    }

    pub(crate) fn with_hvigor_emit_wrapper(mut self) -> Self {
        self.hvigor_emit_wrapper = true;
        self
    }

    /// Match the target selected by latest `ets_checker.js::getTargetESVersion`.
    pub(crate) fn with_tsc_target(mut self, tsc_target: ArkTscTarget) -> Self {
        self.tsc_target = tsc_target;
        self
    }

    pub(crate) fn emit_with_source_map(&self) -> Option<TypeScriptJavascriptEmitOutput> {
        let allocator = Allocator::default();
        let source_type = self.source_type();
        let mut parsed = Parser::new(&allocator, self.source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return None;
        }
        let is_external_module = parsed
            .program
            .body
            .iter()
            .any(Statement::is_module_declaration);
        let is_comment_only = parsed.program.body.is_empty();

        let rewriter = ImportedConstEnumRewriter::new(self.id, self.sources);
        let rewriter = match self.const_enum_exports {
            Some(cache) => rewriter.with_const_enum_export_cache(cache),
            None => rewriter,
        };
        rewriter.rewrite_program(&allocator, &mut parsed.program);

        let scoping = {
            let semantic_ret = SemanticBuilder::new()
                .with_enum_eval(true)
                .build(&parsed.program);
            if !semantic_ret.diagnostics.is_empty() {
                return None;
            }
            semantic_ret.semantic.into_scoping()
        };

        // `ets_checker.js::getTargetESVersion` defaults to ES2021 (or accepts
        // an explicit ES2017). With `useDefineForClassFields: false`, the
        // TypeScript printer emits static field initializers as assignments
        // after the class. Selecting OXC's same target enables its structured
        // class-properties lowering; leaving EnvOptions at its default instead
        // emits ES2022 static blocks and changes es2abc function records.
        let mut options = TransformOptions::from_target(self.tsc_target.es_target())
            .expect("ArkTscTarget always maps to an OXC ES target");
        options.assumptions.set_public_class_fields = true;
        options.typescript.optimize_const_enums = true;
        options.typescript.remove_class_fields_without_initializer = true;
        // Hvigor's compiler options enable TypeScript experimental
        // decorators. Coverage/project-level JavaScript emission must retain
        // property, method, parameter and class decorator side effects rather
        // than merely erasing their TypeScript syntax.
        options.decorator.legacy = true;
        let path = Path::new(self.id);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Transformer::new(&allocator, path, &options)
                .build_with_scoping(scoping, &mut parsed.program)
        }));
        if result.is_err() {
            return None;
        }
        TypeScriptDecoratorHelperCanonicalizer::new(&allocator).canonicalize(&mut parsed.program);
        TypeScriptEmitParityCanonicalizer::new(&allocator).canonicalize(&mut parsed.program);

        let options = CodegenOptions {
            source_map_path: Some(self.id.into()),
            ..CodegenOptions::default()
        };
        let generated = Codegen::new().with_options(options).build(&parsed.program);
        // Latest IDE ets-loader
        // `rollup-plugin-ets-typescript.js::transform` sends a
        // `//@ts-noCheck` input through TypeScript `Program.emit`. For a
        // comment-only generated module such as ResourceTable.ts, that emit
        // intentionally contains no source comments: it is just the emitted
        // strict-mode prologue and source-map reference. OXC codegen preserves
        // attached comments by default, so discard them through the parsed
        // AST's body fact rather than matching generated-file paths or text.
        let generated_code = if self.hvigor_emit_wrapper && is_comment_only {
            String::new()
        } else {
            generated.code
        };
        let generated_line_offset = u32::from(
            self.hvigor_emit_wrapper
                && self.should_prepend_use_strict(&generated_code, is_external_module),
        );
        let source_map = generated.map.as_ref().and_then(|source_map| {
            let source_map = source_map.to_json_string();
            TypeScriptJavaScriptSourceMapBuilder::new(
                &generated_code,
                self.source,
                self.id,
                &source_map,
            )
            .with_generated_line_offset(generated_line_offset)
            .build()
        });
        let code = if self.hvigor_emit_wrapper {
            self.wrap_hvigor_emit(generated_code, is_external_module)
        } else {
            generated_code
        };
        Some(TypeScriptJavascriptEmitOutput { code, source_map })
    }

    fn wrap_hvigor_emit(&self, code: String, is_external_module: bool) -> String {
        let code = code.replace("\r\n", "\n").replace('\r', "\n");
        let code = code.trim();
        let mut sections = Vec::with_capacity(3);
        if self.should_prepend_use_strict(code, is_external_module) {
            sections.push("\"use strict\";".to_string());
        }
        if !code.is_empty() {
            sections.push(code.replace('\n', "\r\n"));
        }
        sections.push(format!(
            "//# sourceMappingURL={}.js.map",
            Path::new(self.id)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("module")
        ));
        sections.join("\r\n")
    }

    fn should_prepend_use_strict(&self, code: &str, is_external_module: bool) -> bool {
        !is_external_module
            && !code.starts_with("\"use strict\";")
            && !code.starts_with("'use strict';")
    }

    fn source_type(&self) -> SourceType {
        CompileArktsSourceType::from_id(self.id)
    }
}

/// Replace OXC's legacy-decorator runtime imports with the inline helpers
/// emitted by the TypeScript compiler used by hvigor.
///
/// OXC owns the decorator lowering itself. This pass only adapts the helper
/// transport boundary: OpenHarmony modules do not depend on
/// `@oxc-project/runtime`, and the CompileArkTS cache must be self-contained.
struct TypeScriptDecoratorHelperCanonicalizer<'a> {
    allocator: &'a Allocator,
    ast: AstBuilder<'a>,
}

impl<'a> TypeScriptDecoratorHelperCanonicalizer<'a> {
    const DECORATE_SOURCE: &'static str = r#"var __decorate = (this && this.__decorate) || function (decorators, target, key, desc) {
  var c = arguments.length, r = c < 3 ? target : desc === null ? desc = Object.getOwnPropertyDescriptor(target, key) : desc, d;
  if (typeof Reflect === "object" && typeof Reflect.decorate === "function") r = Reflect.decorate(decorators, target, key, desc);
  else for (var i = decorators.length - 1; i >= 0; i--) if (d = decorators[i]) r = (c < 3 ? d(r) : c > 3 ? d(target, key, r) : d(target, key)) || r;
  return c > 3 && r && Object.defineProperty(target, key, r), r;
};"#;
    const PARAM_SOURCE: &'static str = r#"var __param = (this && this.__param) || function (paramIndex, decorator) {
  return function (target, key) { decorator(target, key, paramIndex); };
};"#;
    const METADATA_SOURCE: &'static str = r#"var __metadata = (this && this.__metadata) || function (k, v) {
  if (typeof Reflect === "object" && typeof Reflect.metadata === "function") return Reflect.metadata(k, v);
};"#;

    fn new(allocator: &'a Allocator) -> Self {
        Self {
            allocator,
            ast: AstBuilder::new(allocator),
        }
    }

    fn canonicalize(&self, program: &mut Program<'a>) {
        let old_body = std::mem::replace(&mut program.body, ArenaVec::new_in(&self.ast));
        let mut body = ArenaVec::with_capacity_in(old_body.len(), &self.ast);
        let mut aliases = FxHashMap::default();
        let mut helpers = FxHashSet::default();

        for statement in old_body {
            let Some((helper, alias)) = Self::decorator_helper_import(&statement) else {
                body.push(statement);
                continue;
            };
            aliases.insert(alias, helper.binding_name());
            helpers.insert(helper);
        }

        if aliases.is_empty() {
            program.body = body;
            return;
        }

        program.body = body;
        TypeScriptDecoratorHelperReferenceRewriter::new(&self.ast, &aliases).visit_program(program);

        let mut output = ArenaVec::new_in(&self.ast);
        for helper in [
            TypeScriptDecoratorHelper::Decorate,
            TypeScriptDecoratorHelper::Param,
            TypeScriptDecoratorHelper::Metadata,
        ] {
            if helpers.contains(&helper) {
                let parsed =
                    Parser::new(self.allocator, helper.source(), SourceType::mjs()).parse();
                debug_assert!(parsed.diagnostics.is_empty());
                let mut parsed = parsed;
                TypeScriptGeneratedSpanClearer.visit_program(&mut parsed.program);
                output.extend(parsed.program.body);
            }
        }
        output.extend(std::mem::replace(
            &mut program.body,
            ArenaVec::new_in(&self.ast),
        ));
        program.body = output;
    }

    fn decorator_helper_import(
        statement: &Statement<'_>,
    ) -> Option<(TypeScriptDecoratorHelper, String)> {
        let Statement::ImportDeclaration(import) = statement else {
            return None;
        };
        let helper = TypeScriptDecoratorHelper::from_import_source(import.source.value.as_str())?;
        let local = import.specifiers.as_ref()?.iter().find_map(|specifier| {
            let ImportDeclarationSpecifier::ImportDefaultSpecifier(default) = specifier else {
                return None;
            };
            Some(default.local.name.to_string())
        })?;
        Some((helper, local))
    }
}

/// Inline helper source is parsed into the user's allocator, but its offsets
/// belong to the helper snippet rather than the user module. Mark those nodes
/// synthetic so OXC source-map generation never indexes the user source with
/// unrelated helper offsets.
struct TypeScriptGeneratedSpanClearer;

impl<'a> VisitMut<'a> for TypeScriptGeneratedSpanClearer {
    fn visit_span(&mut self, span: &mut oxc_span::Span) {
        *span = SPAN;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum TypeScriptDecoratorHelper {
    Decorate,
    Param,
    Metadata,
}

impl TypeScriptDecoratorHelper {
    fn from_import_source(source: &str) -> Option<Self> {
        match source.rsplit('/').next()? {
            "decorate" => Some(Self::Decorate),
            "decorateParam" => Some(Self::Param),
            "decorateMetadata" => Some(Self::Metadata),
            _ => None,
        }
    }

    fn binding_name(self) -> &'static str {
        match self {
            Self::Decorate => "__decorate",
            Self::Param => "__param",
            Self::Metadata => "__metadata",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::Decorate => TypeScriptDecoratorHelperCanonicalizer::DECORATE_SOURCE,
            Self::Param => TypeScriptDecoratorHelperCanonicalizer::PARAM_SOURCE,
            Self::Metadata => TypeScriptDecoratorHelperCanonicalizer::METADATA_SOURCE,
        }
    }
}

struct TypeScriptDecoratorHelperReferenceRewriter<'a, 'aliases, 'builder> {
    ast: &'builder AstBuilder<'a>,
    aliases: &'aliases FxHashMap<String, &'static str>,
}

impl<'a, 'aliases, 'builder> TypeScriptDecoratorHelperReferenceRewriter<'a, 'aliases, 'builder> {
    fn new(
        ast: &'builder AstBuilder<'a>,
        aliases: &'aliases FxHashMap<String, &'static str>,
    ) -> Self {
        Self { ast, aliases }
    }
}

impl<'a> VisitMut<'a> for TypeScriptDecoratorHelperReferenceRewriter<'a, '_, '_> {
    fn visit_identifier_reference(&mut self, identifier: &mut IdentifierReference<'a>) {
        if let Some(replacement) = self.aliases.get(identifier.name.as_str()) {
            identifier.name = Ident::from_str_in(replacement, self.ast);
        }
    }
}

/// Canonicalize the small, observable gaps between OXC's TypeScript lowering
/// and the latest SDK TypeScript emitter before `es2abc` consumes the cache.
///
/// This remains an OXC AST pass: no generated-source string patches are used.
/// The pass deliberately recognizes only OXC's synthesized enum-IIFE and
/// class-static-alias shapes, so ordinary user-written code is untouched.
struct TypeScriptEmitParityCanonicalizer<'a> {
    ast: AstBuilder<'a>,
}

impl<'a> TypeScriptEmitParityCanonicalizer<'a> {
    fn new(allocator: &'a Allocator) -> Self {
        Self {
            ast: AstBuilder::new(allocator),
        }
    }

    fn canonicalize(&self, program: &mut Program<'a>) {
        self.canonicalize_regular_enum_declarations(program);
        self.canonicalize_decorated_class_aliases(program);
        self.canonicalize_class_static_self_references(program);
        TypeScriptSynthesizedConstructorCanonicalizer::new(&self.ast).visit_program(program);
    }

    fn canonicalize_decorated_class_aliases(&self, program: &mut Program<'a>) {
        let aliases = program
            .body
            .iter()
            .filter_map(Self::variable_declaration)
            .flat_map(|declaration| declaration.declarations.iter())
            .filter_map(Self::decorated_class_alias)
            .collect::<FxHashMap<_, _>>();
        if aliases.is_empty() {
            return;
        }

        let old_body = std::mem::replace(&mut program.body, ArenaVec::new_in(&self.ast));
        let mut body = ArenaVec::with_capacity_in(old_body.len(), &self.ast);
        for mut statement in old_body {
            if self.retain_non_alias_variable_declarators(&mut statement, &aliases) {
                body.push(statement);
            }
        }
        program.body = body;
        OxcDecoratedClassAliasRewriter::new(&self.ast, &aliases).visit_program(program);
    }

    fn variable_declaration<'statement>(
        statement: &'statement Statement<'a>,
    ) -> Option<&'statement VariableDeclaration<'a>> {
        match statement {
            Statement::VariableDeclaration(declaration) => Some(declaration),
            Statement::ExportDeclaration(export) => {
                let Declaration::VariableDeclaration(declaration) = &export.declaration else {
                    return None;
                };
                Some(declaration)
            }
            _ => None,
        }
    }

    fn decorated_class_alias(
        declarator: &oxc_ast::ast::VariableDeclarator<'a>,
    ) -> Option<(String, String)> {
        let class_name = declarator.id.get_binding_identifier()?.name.as_str();
        let Expression::AssignmentExpression(assignment) = declarator.init.as_ref()? else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(alias) = &assignment.left else {
            return None;
        };
        let Expression::ClassExpression(class) = &assignment.right else {
            return None;
        };
        let emitted_name = class.id.as_ref()?.name.as_str();
        (emitted_name == class_name && Self::is_oxc_class_static_alias(&alias.name, class_name))
            .then(|| (alias.name.to_string(), class_name.to_string()))
    }

    /// The SDK TypeScript emitter writes a regular enum as a hoisted `var`
    /// followed by `(function (Enum) { ... })(Enum || (Enum = {}));`. OXC
    /// emits a `let` initialized from a pure IIFE that returns the enum object.
    /// The latter adds different ABC records even though it is value-equivalent.
    fn canonicalize_regular_enum_declarations(&self, program: &mut Program<'a>) {
        let old_body = std::mem::replace(&mut program.body, ArenaVec::new_in(&self.ast));
        let mut body = ArenaVec::with_capacity_in(old_body.len(), &self.ast);
        for mut statement in old_body {
            let enum_iife = self.take_oxc_enum_iife_declaration(&mut statement);
            body.push(statement);
            if let Some(enum_iife) = enum_iife {
                body.push(Statement::new_expression_statement(
                    SPAN, enum_iife, &self.ast,
                ));
            }
        }
        program.body = body;
    }

    fn take_oxc_enum_iife_declaration(
        &self,
        statement: &mut Statement<'a>,
    ) -> Option<Expression<'a>> {
        let declaration = match statement {
            Statement::VariableDeclaration(declaration) => declaration,
            Statement::ExportDeclaration(export) => {
                let Declaration::VariableDeclaration(declaration) = &mut export.declaration else {
                    return None;
                };
                declaration
            }
            _ => return None,
        };
        self.take_oxc_enum_iife_from_variable_declaration(declaration)
    }

    fn take_oxc_enum_iife_from_variable_declaration(
        &self,
        declaration: &mut VariableDeclaration<'a>,
    ) -> Option<Expression<'a>> {
        if declaration.declarations.len() != 1 {
            return None;
        }
        let declarator = &mut declaration.declarations[0];
        let enum_name = declarator
            .id
            .get_binding_identifier()
            .map(|identifier| identifier.name.to_string())?;
        if !Self::is_oxc_enum_iife(declarator.init.as_ref()?, &enum_name) {
            return None;
        }
        let Expression::CallExpression(mut call) = declarator
            .init
            .take()
            .expect("validated OXC enum IIFE initializer")
        else {
            unreachable!("validated OXC enum IIFE initializer")
        };

        let function = match &mut call.callee {
            Expression::FunctionExpression(function) => function,
            _ => unreachable!("validated OXC enum IIFE callee"),
        };
        function
            .body
            .as_mut()
            .expect("validated OXC enum IIFE body")
            .statements
            .pop();
        call.pure = false;
        let enum_identifier = || Ident::from_str_in(enum_name.as_str(), &self.ast);
        call.arguments = ArenaVec::from_value_in(
            Argument::from(Expression::new_logical_expression(
                SPAN,
                Expression::new_identifier(SPAN, enum_identifier(), &self.ast),
                LogicalOperator::Or,
                Expression::new_assignment_expression(
                    SPAN,
                    AssignmentOperator::Assign,
                    AssignmentTarget::new_assignment_target_identifier(
                        SPAN,
                        enum_identifier(),
                        &self.ast,
                    ),
                    Expression::new_object_expression(SPAN, ArenaVec::new_in(&self.ast), &self.ast),
                    &self.ast,
                ),
                &self.ast,
            )),
            &self.ast,
        );
        declaration.kind = VariableDeclarationKind::Var;
        Some(Expression::CallExpression(call))
    }

    fn is_oxc_enum_iife(expression: &Expression<'_>, enum_name: &str) -> bool {
        let Expression::CallExpression(call) = expression else {
            return false;
        };
        let [argument] = call.arguments.as_slice() else {
            return false;
        };
        if !Self::is_oxc_enum_iife_argument(argument, enum_name) {
            return false;
        }
        let Expression::FunctionExpression(function) = &call.callee else {
            return false;
        };
        let Some(parameter) = function
            .params
            .items
            .as_slice()
            .first()
            .and_then(|parameter| parameter.pattern.get_binding_identifier())
        else {
            return false;
        };
        if function.id.is_some()
            || function.params.items.len() != 1
            || parameter.name.as_str() != enum_name
        {
            return false;
        }
        let Some(body) = &function.body else {
            return false;
        };
        matches!(
            body.statements.last(),
            Some(Statement::ReturnStatement(statement))
                if matches!(
                    &statement.argument,
                    Some(Expression::Identifier(identifier)) if identifier.name.as_str() == enum_name
                )
        )
    }

    fn is_oxc_enum_iife_argument(argument: &Argument<'_>, enum_name: &str) -> bool {
        match argument {
            Argument::ObjectExpression(object) => object.properties.is_empty(),
            Argument::LogicalExpression(logical)
                if logical.operator == LogicalOperator::Or
                    && matches!(
                        &logical.left,
                        Expression::Identifier(identifier) if identifier.name == enum_name
                    ) =>
            {
                matches!(&logical.right, Expression::ObjectExpression(object) if object.properties.is_empty())
            }
            _ => false,
        }
    }

    /// OXC's class-fields lowering creates `var _Class; _Class = Class;` for
    /// a declaration whose static initializer refers to its own name. The SDK
    /// TypeScript emitter writes the direct class reference in its post-class
    /// assignment instead. Canonicalize only the exact synthesized pair and
    /// only inside assignments to that class's static members.
    fn canonicalize_class_static_self_references(&self, program: &mut Program<'a>) {
        let aliases = self.collect_oxc_class_static_aliases(program);
        if aliases.is_empty() {
            return;
        }

        let old_body = std::mem::replace(&mut program.body, ArenaVec::new_in(&self.ast));
        let mut body = ArenaVec::with_capacity_in(old_body.len(), &self.ast);
        for mut statement in old_body {
            if Self::is_oxc_class_alias_assignment(&statement, &aliases)
                || !self.retain_non_alias_variable_declarators(&mut statement, &aliases)
            {
                continue;
            }
            self.rewrite_oxc_class_static_assignment(&mut statement, &aliases);
            body.push(statement);
        }
        program.body = body;
    }

    fn collect_oxc_class_static_aliases(&self, program: &Program<'a>) -> FxHashMap<String, String> {
        let class_names = program
            .body
            .iter()
            .filter_map(Self::declared_class_name)
            .collect::<FxHashSet<_>>();
        if class_names.is_empty() {
            return FxHashMap::default();
        }

        let alias_declarations = program
            .body
            .iter()
            .filter_map(Self::uninitialized_var_declaration)
            .flat_map(|declaration| declaration.declarations.iter())
            .filter(|declarator| declarator.init.is_none())
            .filter_map(|declarator| {
                declarator
                    .id
                    .get_binding_identifier()
                    .map(|identifier| identifier.name.to_string())
            })
            .collect::<FxHashSet<_>>();
        if alias_declarations.is_empty() {
            return FxHashMap::default();
        }

        program
            .body
            .iter()
            .filter_map(Self::simple_identifier_assignment)
            .filter(|(alias, class_name)| {
                alias_declarations.contains(alias)
                    && class_names.contains(class_name)
                    && Self::is_oxc_class_static_alias(alias, class_name)
            })
            .collect()
    }

    fn declared_class_name(statement: &Statement<'a>) -> Option<String> {
        match statement {
            Statement::ClassDeclaration(class) => class
                .id
                .as_ref()
                .map(|identifier| identifier.name.to_string()),
            Statement::ExportDeclaration(export) => {
                let Declaration::ClassDeclaration(class) = &export.declaration else {
                    return None;
                };
                class
                    .id
                    .as_ref()
                    .map(|identifier| identifier.name.to_string())
            }
            Statement::ExportDefaultDeclaration(export) => {
                let ExportDefaultDeclarationKind::ClassDeclaration(class) = &export.declaration
                else {
                    return None;
                };
                class
                    .id
                    .as_ref()
                    .map(|identifier| identifier.name.to_string())
            }
            _ => None,
        }
    }

    fn uninitialized_var_declaration<'statement>(
        statement: &'statement Statement<'a>,
    ) -> Option<&'statement VariableDeclaration<'a>> {
        let Statement::VariableDeclaration(declaration) = statement else {
            return None;
        };
        (declaration.kind == VariableDeclarationKind::Var).then_some(declaration)
    }

    fn simple_identifier_assignment(statement: &Statement<'a>) -> Option<(String, String)> {
        let Statement::ExpressionStatement(statement) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &statement.expression else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(left) = &assignment.left else {
            return None;
        };
        let Expression::Identifier(right) = &assignment.right else {
            return None;
        };
        Some((left.name.to_string(), right.name.to_string()))
    }

    fn is_oxc_class_static_alias(alias: &str, class_name: &str) -> bool {
        let Some(alias) = alias.strip_prefix('_') else {
            return false;
        };
        let Some(disambiguator) = alias.strip_prefix(class_name) else {
            return false;
        };
        disambiguator.is_empty() || disambiguator.bytes().all(|byte| byte.is_ascii_digit())
    }

    fn is_oxc_class_alias_assignment(
        statement: &Statement<'a>,
        aliases: &FxHashMap<String, String>,
    ) -> bool {
        Self::simple_identifier_assignment(statement)
            .is_some_and(|(alias, class_name)| aliases.get(&alias) == Some(&class_name))
    }

    fn retain_non_alias_variable_declarators(
        &self,
        statement: &mut Statement<'a>,
        aliases: &FxHashMap<String, String>,
    ) -> bool {
        let Statement::VariableDeclaration(declaration) = statement else {
            return true;
        };
        if declaration.kind != VariableDeclarationKind::Var {
            return true;
        }

        let old_declarations =
            std::mem::replace(&mut declaration.declarations, ArenaVec::new_in(&self.ast));
        let mut declarations = ArenaVec::with_capacity_in(old_declarations.len(), &self.ast);
        for declarator in old_declarations {
            let is_alias = declarator.init.is_none()
                && declarator
                    .id
                    .get_binding_identifier()
                    .is_some_and(|identifier| aliases.contains_key(identifier.name.as_str()));
            if !is_alias {
                declarations.push(declarator);
            }
        }
        declaration.declarations = declarations;
        !declaration.declarations.is_empty()
    }

    fn rewrite_oxc_class_static_assignment(
        &self,
        statement: &mut Statement<'a>,
        aliases: &FxHashMap<String, String>,
    ) {
        let Statement::ExpressionStatement(statement) = statement else {
            return;
        };
        let Expression::AssignmentExpression(assignment) = &mut statement.expression else {
            return;
        };
        let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
            return;
        };
        let Expression::Identifier(class_identifier) = &member.object else {
            return;
        };
        let class_name = class_identifier.name.to_string();
        if !aliases.values().any(|target| target == &class_name) {
            return;
        }

        let mut rewriter =
            OxcClassStaticAliasReferenceRewriter::new(&self.ast, aliases, class_name.as_str());
        rewriter.visit_expression(&mut assignment.right);
    }
}

/// OXC inserts `constructor(..._args) { super(..._args); ... }` while the
/// TypeScript emitter used by hvigor inserts
/// `constructor() { super(...arguments); ... }`. Only constructors whose own
/// span and rest parameter are synthetic are eligible, so authored rest
/// parameters retain their original semantics.
struct TypeScriptSynthesizedConstructorCanonicalizer<'a, 'builder> {
    ast: &'builder AstBuilder<'a>,
}

impl<'a, 'builder> TypeScriptSynthesizedConstructorCanonicalizer<'a, 'builder> {
    fn new(ast: &'builder AstBuilder<'a>) -> Self {
        Self { ast }
    }

    fn canonicalize(&self, method: &mut MethodDefinition<'a>) {
        if method.kind != MethodDefinitionKind::Constructor || method.span != SPAN {
            return;
        }
        let Some(rest) = method.value.params.rest.as_ref() else {
            return;
        };
        if rest.span != SPAN {
            return;
        }
        let Some(rest_name) = rest.rest.argument.get_binding_identifier() else {
            return;
        };
        let rest_name = rest_name.name.to_string();
        let Some(body) = method.value.body.as_mut() else {
            return;
        };
        let Some(Statement::ExpressionStatement(statement)) = body.statements.first_mut() else {
            return;
        };
        let Expression::CallExpression(call) = &mut statement.expression else {
            return;
        };
        if !call.callee.is_super() || call.arguments.len() != 1 {
            return;
        }
        let Argument::SpreadElement(spread) = &mut call.arguments[0] else {
            return;
        };
        let Expression::Identifier(argument) = &mut spread.argument else {
            return;
        };
        if argument.name.as_str() != rest_name {
            return;
        }

        argument.name = Ident::from_str_in("arguments", self.ast);
        method.value.params.rest = None;
    }
}

impl<'a> VisitMut<'a> for TypeScriptSynthesizedConstructorCanonicalizer<'a, '_> {
    fn visit_method_definition(&mut self, method: &mut MethodDefinition<'a>) {
        self.canonicalize(method);
        walk_mut::walk_method_definition(self, method);
    }
}

struct OxcClassStaticAliasReferenceRewriter<'ast, 'aliases, 'class, 'builder> {
    ast: &'builder AstBuilder<'ast>,
    aliases: &'aliases FxHashMap<String, String>,
    class_name: &'class str,
}

struct OxcDecoratedClassAliasRewriter<'ast, 'aliases, 'builder> {
    ast: &'builder AstBuilder<'ast>,
    aliases: &'aliases FxHashMap<String, String>,
}

impl<'ast, 'aliases, 'builder> OxcDecoratedClassAliasRewriter<'ast, 'aliases, 'builder> {
    fn new(ast: &'builder AstBuilder<'ast>, aliases: &'aliases FxHashMap<String, String>) -> Self {
        Self { ast, aliases }
    }
}

impl<'ast> VisitMut<'ast> for OxcDecoratedClassAliasRewriter<'ast, '_, '_> {
    fn visit_expression(&mut self, expression: &mut Expression<'ast>) {
        if let Expression::AssignmentExpression(assignment) = expression {
            if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                if self.aliases.contains_key(identifier.name.as_str()) {
                    *expression = assignment.right.clone_in(self.ast.allocator());
                }
            }
        }
        walk_mut::walk_expression(self, expression);
    }

    fn visit_identifier_reference(&mut self, identifier: &mut IdentifierReference<'ast>) {
        if let Some(class_name) = self.aliases.get(identifier.name.as_str()) {
            identifier.name = Ident::from_str_in(class_name, self.ast);
        }
    }
}

impl<'ast, 'aliases, 'class, 'builder>
    OxcClassStaticAliasReferenceRewriter<'ast, 'aliases, 'class, 'builder>
{
    fn new(
        ast: &'builder AstBuilder<'ast>,
        aliases: &'aliases FxHashMap<String, String>,
        class_name: &'class str,
    ) -> Self {
        Self {
            ast,
            aliases,
            class_name,
        }
    }
}

impl<'ast> VisitMut<'ast> for OxcClassStaticAliasReferenceRewriter<'ast, '_, '_, '_> {
    fn visit_identifier_reference(&mut self, identifier: &mut IdentifierReference<'ast>) {
        if self
            .aliases
            .get(identifier.name.as_str())
            .is_some_and(|target| target == self.class_name)
        {
            identifier.name = Ident::from_str_in(self.class_name, self.ast);
        }
    }
}

/// Parsed TypeScript pragma facts used by the CompileArkTS cache writer.
///
/// This deliberately reads OXC's comment AST instead of searching arbitrary
/// source strings, so string literals and unrelated comments cannot change a
/// module's output language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TypeScriptDirectiveFlags {
    suppress_type_check: bool,
    keep_typescript: bool,
    shared_module: bool,
    comment_only: bool,
}

impl TypeScriptDirectiveFlags {
    pub fn from_source(source: &str, id: &str) -> Self {
        let allocator = Allocator::default();
        let source_type = CompileArktsSourceType::from_id(id);
        let parsed = Parser::new(&allocator, source, source_type).parse();
        Self::from_program(&parsed.program)
    }

    pub fn should_emit_javascript(self) -> bool {
        self.suppress_type_check && !self.keep_typescript
    }

    /// Match SDK `check_shared_module.ts::collectSharedModule`: imports may
    /// precede `"use shared"`, but any other statement ends directive lookup.
    pub fn is_shared_module(self) -> bool {
        self.shared_module
    }

    pub(crate) fn is_comment_only(self) -> bool {
        self.comment_only
    }

    fn from_program(program: &oxc_ast::ast::Program<'_>) -> Self {
        let mut flags = Self::default();
        for comment in &program.comments {
            let span = comment.content_span();
            let Some(content) = program
                .source_text
                .get(span.start as usize..span.end as usize)
            else {
                continue;
            };
            let directive = TypeScriptCommentDirective::new(content, comment.is_line());
            flags.suppress_type_check |= directive.suppresses_type_check();
            flags.keep_typescript |= comment.is_line()
                && program
                    .source_text
                    .get(..comment.span.start as usize)
                    .is_some_and(|prefix| prefix.trim().is_empty())
                && directive.is_keep_typescript();
        }
        flags.shared_module = program
            .body
            .iter()
            .find(|statement| !matches!(statement, oxc_ast::ast::Statement::ImportDeclaration(_)))
            .is_some_and(|statement| {
                let oxc_ast::ast::Statement::ExpressionStatement(statement) = statement else {
                    return false;
                };
                matches!(
                    &statement.expression,
                    oxc_ast::ast::Expression::StringLiteral(literal)
                        if literal.value == "use shared"
                )
            });
        flags.comment_only = program.body.is_empty();
        flags
    }
}

struct TypeScriptCommentDirective<'a> {
    final_line: &'a str,
    line_comment: bool,
}

impl<'a> TypeScriptCommentDirective<'a> {
    fn new(content: &'a str, line_comment: bool) -> Self {
        Self {
            final_line: content.rsplit('\n').next().unwrap_or(content).trim_start(),
            line_comment,
        }
    }

    fn suppresses_type_check(&self) -> bool {
        self.directive("@ts-ignore") || (self.line_comment && self.directive("@ts-nocheck"))
    }

    fn is_keep_typescript(&self) -> bool {
        self.line_comment && self.final_line.trim() == "@keepTs"
    }

    fn directive(&self, name: &str) -> bool {
        let line = self
            .final_line
            .trim_start_matches(|character: char| character == '*' || character.is_whitespace());
        let Some(prefix) = line.get(..name.len()) else {
            return false;
        };
        if !prefix.eq_ignore_ascii_case(name) {
            return false;
        }
        let rest = line.get(name.len()..).unwrap_or_default();
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|character| character == ':' || character.is_whitespace())
    }
}

pub(crate) struct ImportedConstEnumRewriter<'a> {
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
    const_enum_exports: Option<&'a ConstEnumExportCache>,
}

/// Facts retained from the SDK const-enum pass for its later import pass.
///
/// `transformTypeExportImportAndConstEnumInTypeScript::visitImportSpecifier`
/// asks its emit resolver whether an alias was referenced before it replaces
/// the corresponding const-enum property access with a literal. Keep that
/// dependency fact separately because it is intentionally no longer present
/// in the rewritten OXC AST.
#[derive(Debug, Default)]
pub(crate) struct ImportedConstEnumRewriteResult {
    pub(crate) changed: bool,
    pub(crate) inlined_import_locals: FxHashSet<String>,
}

/// Preserve the SDK TypeScript emitter's local enum substitutions in
/// CompileArkTS source. Latest
/// `typescript.js::transformTypeExportImportAndConstEnumInTypeScript` asks its
/// emit resolver to normalize every enum-member initializer and to replace a
/// resolved local `const enum` property/element access with a newly-created
/// literal before `es2abc` reads the cache file. Local binding identity is
/// available from OXC semantic analysis, so use symbols rather than names and
/// never substitute a shadowing value parameter or local variable.
pub(crate) struct TypeScriptEnumValueRewriter;

impl TypeScriptEnumValueRewriter {
    pub(crate) fn rewrite_program<'ast>(
        allocator: &'ast Allocator,
        program: &mut oxc_ast::ast::Program<'ast>,
        scoping: &oxc_semantic::Scoping,
    ) -> bool {
        let enums = LocalEnumValueCollector::new(scoping).collect(program);
        if enums.is_empty() {
            return false;
        }

        let mut visitor = LocalEnumValueMutator {
            ast: AstBuilder::new(allocator),
            scoping,
            enums,
            current_enum: None,
            changed: false,
        };
        visitor.visit_program(program);
        visitor.changed
    }
}

#[derive(Debug, Clone)]
struct LocalEnumValues {
    members: FxHashMap<String, EnumValue>,
    is_const: bool,
}

struct LocalEnumValueCollector<'s> {
    scoping: &'s oxc_semantic::Scoping,
    enums: FxHashMap<SymbolId, LocalEnumValues>,
}

impl<'s> LocalEnumValueCollector<'s> {
    fn new(scoping: &'s oxc_semantic::Scoping) -> Self {
        Self {
            scoping,
            enums: FxHashMap::default(),
        }
    }

    fn collect<'ast>(
        mut self,
        program: &oxc_ast::ast::Program<'ast>,
    ) -> FxHashMap<SymbolId, LocalEnumValues> {
        self.visit_program(program);
        self.enums
    }
}

impl<'ast> Visit<'ast> for LocalEnumValueCollector<'_> {
    fn visit_ts_enum_declaration(&mut self, declaration: &oxc_ast::ast::TSEnumDeclaration<'ast>) {
        if let Some(members) = ConstEnumMemberCollector::new(declaration, self.scoping).collect() {
            self.enums.insert(
                declaration.id.symbol_id(),
                LocalEnumValues {
                    members,
                    is_const: declaration.r#const,
                },
            );
        }
        walk::walk_ts_enum_declaration(self, declaration);
    }
}

struct LocalEnumValueMutator<'ast, 'scope> {
    ast: AstBuilder<'ast>,
    scoping: &'scope oxc_semantic::Scoping,
    enums: FxHashMap<SymbolId, LocalEnumValues>,
    current_enum: Option<SymbolId>,
    changed: bool,
}

impl<'ast> VisitMut<'ast> for LocalEnumValueMutator<'ast, '_> {
    fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
        if let Some(replacement) = self.const_enum_replacement(expr).cloned() {
            *expr = ConstEnumValueEmitter::new(&replacement).expression(&self.ast);
            self.changed = true;
            return;
        }
        walk_mut::walk_expression(self, expr);
    }

    fn visit_ts_enum_declaration(
        &mut self,
        declaration: &mut oxc_ast::ast::TSEnumDeclaration<'ast>,
    ) {
        let previous = self.current_enum;
        self.current_enum = Some(declaration.id.symbol_id());
        walk_mut::walk_ts_enum_declaration(self, declaration);
        self.current_enum = previous;
    }

    fn visit_ts_enum_member(&mut self, member: &mut oxc_ast::ast::TSEnumMember<'ast>) {
        if let Some(replacement) = self.enum_member_replacement(member).cloned() {
            member.initializer =
                Some(ConstEnumValueEmitter::new(&replacement).expression(&self.ast));
            self.changed = true;
            return;
        }
        walk_mut::walk_ts_enum_member(self, member);
    }
}

impl LocalEnumValueMutator<'_, '_> {
    fn const_enum_replacement<'a>(&'a self, expr: &Expression<'_>) -> Option<&'a EnumValue> {
        match expr {
            Expression::StaticMemberExpression(member) => self.static_member_replacement(member),
            Expression::ComputedMemberExpression(member) => {
                self.computed_member_replacement(member)
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::StaticMemberExpression(member) => {
                    self.static_member_replacement(member)
                }
                ChainElement::ComputedMemberExpression(member) => {
                    self.computed_member_replacement(member)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn static_member_replacement<'a>(
        &'a self,
        member: &StaticMemberExpression<'_>,
    ) -> Option<&'a EnumValue> {
        self.member_replacement(&member.object, member.property.name.as_str())
    }

    fn computed_member_replacement<'a>(
        &'a self,
        member: &ComputedMemberExpression<'_>,
    ) -> Option<&'a EnumValue> {
        let Expression::StringLiteral(property) = &member.expression else {
            return None;
        };
        self.member_replacement(&member.object, property.value.as_str())
    }

    fn member_replacement<'a>(
        &'a self,
        object: &Expression<'_>,
        member_name: &str,
    ) -> Option<&'a EnumValue> {
        let Expression::Identifier(identifier) = object else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        let enum_values = self.enums.get(&symbol_id)?;
        enum_values
            .is_const
            .then(|| enum_values.members.get(member_name))?
    }

    fn enum_member_replacement<'a>(
        &'a self,
        member: &oxc_ast::ast::TSEnumMember<'_>,
    ) -> Option<&'a EnumValue> {
        let enum_symbol = self.current_enum?;
        self.enums
            .get(&enum_symbol)?
            .members
            .get(member.id.static_name().as_ref())
    }
}

impl<'a> ImportedConstEnumRewriter<'a> {
    pub(crate) fn new(id: &'a str, sources: &'a TransformSourceSnapshot) -> Self {
        Self {
            id,
            sources,
            const_enum_exports: None,
        }
    }

    pub(crate) fn with_const_enum_export_cache(mut self, cache: &'a ConstEnumExportCache) -> Self {
        self.const_enum_exports = Some(cache);
        self
    }

    pub(crate) fn rewrite_program<'ast>(
        &self,
        allocator: &'ast Allocator,
        program: &mut oxc_ast::ast::Program<'ast>,
    ) -> bool {
        self.rewrite_program_with_inlined_imports(allocator, program)
            .changed
    }

    pub(crate) fn rewrite_program_with_inlined_imports<'ast>(
        &self,
        allocator: &'ast Allocator,
        program: &mut oxc_ast::ast::Program<'ast>,
    ) -> ImportedConstEnumRewriteResult {
        let imports = {
            let collector = ConstEnumImportCollector::new(&program.body, self.id, self.sources);
            let collector = collector.with_const_enum_export_cache_opt(self.const_enum_exports);
            collector.collect()
        };
        if imports.is_empty() {
            return ImportedConstEnumRewriteResult::default();
        }

        let mut visitor = ConstEnumInlineMutator {
            ast: AstBuilder::new(allocator),
            imports,
            in_enum_initializer: false,
            changed: false,
            inlined_import_locals: FxHashSet::default(),
        };
        visitor.visit_program(program);
        ImportedConstEnumRewriteResult {
            changed: visitor.changed,
            inlined_import_locals: visitor.inlined_import_locals,
        }
    }
}

struct ConstEnumImportCollector<'a> {
    statements: &'a [Statement<'a>],
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
    const_enum_exports: Option<&'a ConstEnumExportCache>,
}

impl<'a> ConstEnumImportCollector<'a> {
    fn new(
        statements: &'a [Statement<'a>],
        id: &'a str,
        sources: &'a TransformSourceSnapshot,
    ) -> Self {
        Self {
            statements,
            id,
            sources,
            const_enum_exports: None,
        }
    }

    fn with_const_enum_export_cache_opt(mut self, cache: Option<&'a ConstEnumExportCache>) -> Self {
        self.const_enum_exports = cache;
        self
    }

    fn collect(&self) -> FxHashMap<String, ConstEnumImport> {
        let mut imports = FxHashMap::default();
        for statement in self.statements {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            self.collect_import(import, &mut imports);
        }
        imports
    }

    fn collect_import(
        &self,
        import: &ImportDeclaration<'_>,
        imports: &mut FxHashMap<String, ConstEnumImport>,
    ) {
        if import.import_kind.is_type() {
            return;
        }
        let Some(target) = self
            .sources
            .resolve_static_with_declarations(Path::new(self.id), import.source.value.as_str())
        else {
            return;
        };
        let exports = if let Some(cache) = self.const_enum_exports {
            cache.analyze(target)
        } else {
            ConstEnumExportAnalyzer::new(target)
                .with_sources(self.sources)
                .analyze()
        };
        let Some(exports) = exports else {
            return;
        };
        let Some(specifiers) = &import.specifiers else {
            return;
        };

        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    let imported = ModuleExportNameText::new(&specifier.imported).binding_name();
                    let local = specifier.local.name.to_string();
                    if let Some(members) = exports.enums.get(&imported) {
                        imports.insert(local, ConstEnumImport::Enum(members.clone()));
                    } else if let Some(namespace) = exports.namespaces.get(&imported) {
                        imports.insert(local, ConstEnumImport::Namespace(namespace.clone()));
                    }
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    imports.insert(
                        specifier.local.name.to_string(),
                        ConstEnumImport::Namespace(exports.enums.clone()),
                    );
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    let local = specifier.local.name.to_string();
                    if let Some(members) = exports.enums.get("default") {
                        imports.insert(local, ConstEnumImport::Enum(members.clone()));
                    } else if let Some(namespace) = exports.namespaces.get("default") {
                        imports.insert(local, ConstEnumImport::Namespace(namespace.clone()));
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConstEnumExportCache {
    sources: TransformSourceSnapshot,
    entries: RwLock<FxHashMap<PathBuf, Option<ConstEnumExports>>>,
}

impl ConstEnumExportCache {
    pub(crate) fn new(sources: TransformSourceSnapshot) -> Self {
        Self {
            sources,
            entries: RwLock::new(FxHashMap::default()),
        }
    }

    fn analyze(&self, source: TransformSourceFile) -> Option<ConstEnumExports> {
        let mut visited = FxHashSet::default();
        self.analyze_with_visited(source, &mut visited)
    }

    fn analyze_with_visited(
        &self,
        source: TransformSourceFile,
        visited: &mut FxHashSet<PathBuf>,
    ) -> Option<ConstEnumExports> {
        let source_path = source.path.clone();
        if visited.contains(&source_path) {
            return None;
        }
        if let Some(cached) = self
            .entries
            .read()
            .ok()
            .and_then(|entries| entries.get(&source_path).cloned())
        {
            return cached;
        }

        let exports = ConstEnumExportAnalyzer::new(source)
            .with_sources(&self.sources)
            .with_cache(self)
            .analyze_with_visited(visited);
        if let Ok(mut entries) = self.entries.write() {
            entries
                .entry(source_path)
                .or_insert_with(|| exports.clone());
        }
        exports
    }
}

struct ConstEnumExportAnalyzer<'a> {
    source: TransformSourceFile,
    sources: Option<&'a TransformSourceSnapshot>,
    cache: Option<&'a ConstEnumExportCache>,
}

impl<'a> ConstEnumExportAnalyzer<'a> {
    fn new(source: TransformSourceFile) -> Self {
        Self {
            source,
            sources: None,
            cache: None,
        }
    }

    fn with_sources(mut self, sources: &'a TransformSourceSnapshot) -> Self {
        self.sources = Some(sources);
        self
    }

    fn with_cache(mut self, cache: &'a ConstEnumExportCache) -> Self {
        self.cache = Some(cache);
        self
    }

    fn analyze(&self) -> Option<ConstEnumExports> {
        let mut visited = FxHashSet::default();
        self.analyze_with_visited(&mut visited)
    }

    fn analyze_with_visited(&self, visited: &mut FxHashSet<PathBuf>) -> Option<ConstEnumExports> {
        let source_path = self.source.path.clone();
        if !visited.insert(source_path.clone()) {
            return None;
        }

        let allocator = Allocator::default();
        let source_type = CompileArktsSourceType::from_path(&self.source.path);
        let parsed = Parser::new(&allocator, &self.source.source, source_type).parse();
        let include_ambient_enums =
            SourceClassifier::classify_path(&self.source.path).is_declaration();
        // SDK `ets_checker.js::printArkTSLinterDiagnostic` downgrades errors in
        // SDK declarations to warnings and keeps the TypeScript program alive.
        // Some shipped declarations contain recoverable constructs such as a
        // redundant `declare` inside an ambient namespace. OXC still provides
        // the complete declaration AST, so retain it for enum metadata while
        // keeping non-declaration source analysis strict.
        if !parsed.diagnostics.is_empty() && !include_ambient_enums {
            visited.remove(&source_path);
            return None;
        }
        let semantic_ret = SemanticBuilder::new()
            .with_enum_eval(true)
            .build(&parsed.program);
        let scoping = semantic_ret
            .diagnostics
            .is_empty()
            .then(|| semantic_ret.semantic.scoping());
        let imported_bindings = self.collect_imported_bindings(&parsed.program.body, visited);

        let mut local_namespaces = FxHashMap::default();
        for statement in &parsed.program.body {
            let declaration = match statement {
                Statement::TSNamespaceDeclaration(declaration) => Some(declaration.as_ref()),
                Statement::ExportDeclaration(export) => match &export.declaration {
                    Declaration::TSNamespaceDeclaration(declaration) => Some(declaration.as_ref()),
                    _ => None,
                },
                _ => None,
            };
            let Some(declaration) = declaration else {
                continue;
            };
            let Some(scoping) = scoping else {
                continue;
            };
            if let Some(namespace) =
                ConstEnumNamespaceCollector::new(declaration, scoping, include_ambient_enums)
                    .collect()
            {
                local_namespaces.insert(namespace.name, namespace.enums);
            }
        }

        let mut exports = ConstEnumExports::default();
        for statement in &parsed.program.body {
            match statement {
                Statement::ExportDeclaration(export) => {
                    self.collect_declaration(
                        &export.declaration,
                        scoping,
                        include_ambient_enums,
                        &mut exports,
                    );
                }
                Statement::ExportNamedDeclaration(export) => {
                    self.collect_local_named_export(export, &imported_bindings, &mut exports);
                }
                Statement::ExportFromDeclaration(export) => {
                    self.collect_reexport(export, visited, &mut exports);
                }
                Statement::ExportDefaultDeclaration(export) => {
                    let oxc_ast::ast::ExportDefaultDeclarationKind::Identifier(identifier) =
                        &export.declaration
                    else {
                        continue;
                    };
                    if let Some(namespace) = local_namespaces.get(identifier.name.as_str()) {
                        exports
                            .namespaces
                            .insert("default".to_string(), namespace.clone());
                    } else if let Some(binding) = imported_bindings.get(identifier.name.as_str()) {
                        Self::insert_export_binding(
                            &mut exports,
                            "default".to_string(),
                            binding.clone(),
                        );
                    }
                }
                Statement::ExportAllDeclaration(export) => {
                    self.collect_export_all(export, visited, &mut exports);
                }
                _ => {}
            }
        }
        visited.remove(&source_path);
        Some(exports)
    }

    fn collect_imported_bindings(
        &self,
        statements: &[Statement<'_>],
        visited: &mut FxHashSet<PathBuf>,
    ) -> FxHashMap<String, ConstEnumImport> {
        let mut bindings = FxHashMap::default();
        for statement in statements {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.import_kind.is_type() {
                continue;
            }
            let Some(target_exports) =
                self.analyze_reexport_source(import.source.value.as_str(), visited)
            else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        if specifier.import_kind.is_type() {
                            continue;
                        }
                        let imported =
                            ModuleExportNameText::new(&specifier.imported).binding_name();
                        let local = specifier.local.name.to_string();
                        if let Some(imported_enum) = target_exports.enums.get(&imported) {
                            bindings.insert(local, ConstEnumImport::Enum(imported_enum.clone()));
                        } else if let Some(namespace) = target_exports.namespaces.get(&imported) {
                            bindings.insert(local, ConstEnumImport::Namespace(namespace.clone()));
                        }
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        bindings.insert(
                            specifier.local.name.to_string(),
                            ConstEnumImport::Namespace(target_exports.enums.clone()),
                        );
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        let local = specifier.local.name.to_string();
                        if let Some(imported_enum) = target_exports.enums.get("default") {
                            bindings.insert(local, ConstEnumImport::Enum(imported_enum.clone()));
                        } else if let Some(namespace) = target_exports.namespaces.get("default") {
                            bindings.insert(local, ConstEnumImport::Namespace(namespace.clone()));
                        }
                    }
                }
            }
        }
        bindings
    }

    fn collect_local_named_export(
        &self,
        export: &ExportNamedDeclaration<'_>,
        imported_bindings: &FxHashMap<String, ConstEnumImport>,
        exports: &mut ConstEnumExports,
    ) {
        for specifier in &export.specifiers {
            if export.export_kind.is_type() || specifier.export_kind.is_type() {
                continue;
            }
            let local_name = ModuleExportNameText::new(&specifier.local).binding_name();
            let exported_name = ModuleExportNameText::new(&specifier.exported).binding_name();
            if let Some(binding) = imported_bindings.get(&local_name) {
                Self::insert_export_binding(exports, exported_name, binding.clone());
            }
        }
    }

    fn collect_reexport(
        &self,
        export: &ExportFromDeclaration<'_>,
        visited: &mut FxHashSet<PathBuf>,
        exports: &mut ConstEnumExports,
    ) {
        let Some(target_exports) =
            self.analyze_reexport_source(export.source.value.as_str(), visited)
        else {
            return;
        };
        for specifier in &export.specifiers {
            if export.export_kind.is_type() || specifier.export_kind.is_type() {
                continue;
            }
            let local_name = ModuleExportNameText::new(&specifier.local).binding_name();
            let exported_name = ModuleExportNameText::new(&specifier.exported).binding_name();
            if let Some(members) = target_exports.enums.get(&local_name) {
                exports.enums.insert(exported_name, members.clone());
            } else if let Some(namespace) = target_exports.namespaces.get(&local_name) {
                exports.namespaces.insert(exported_name, namespace.clone());
            }
        }
    }

    fn insert_export_binding(
        exports: &mut ConstEnumExports,
        exported_name: String,
        binding: ConstEnumImport,
    ) {
        match binding {
            ConstEnumImport::Enum(imported_enum) => {
                exports.enums.insert(exported_name, imported_enum);
            }
            ConstEnumImport::Namespace(namespace) => {
                exports.namespaces.insert(exported_name, namespace);
            }
        }
    }

    fn collect_export_all(
        &self,
        export: &ExportAllDeclaration<'_>,
        visited: &mut FxHashSet<PathBuf>,
        exports: &mut ConstEnumExports,
    ) {
        let Some(target_exports) =
            self.analyze_reexport_source(export.source.value.as_str(), visited)
        else {
            return;
        };
        if let Some(exported) = &export.exported {
            exports.namespaces.insert(
                ModuleExportNameText::new(exported).binding_name(),
                target_exports.enums,
            );
            return;
        }
        exports.merge(target_exports);
    }

    fn collect_declaration(
        &self,
        declaration: &Declaration<'_>,
        scoping: Option<&oxc_semantic::Scoping>,
        include_ambient_enums: bool,
        exports: &mut ConstEnumExports,
    ) {
        match declaration {
            Declaration::TSEnumDeclaration(declaration)
                if declaration.r#const || include_ambient_enums =>
            {
                let members = scoping
                    .and_then(|scoping| {
                        ConstEnumMemberCollector::new(declaration, scoping).collect()
                    })
                    .or_else(|| ConstEnumAstMemberCollector::new(declaration).collect());
                if let Some(members) = members {
                    let imported_enum = if declaration.r#const {
                        ImportedEnum::const_enum(members)
                    } else {
                        ImportedEnum::ambient_enum(members)
                    };
                    exports
                        .enums
                        .insert(declaration.id.name.to_string(), imported_enum);
                }
            }
            Declaration::TSNamespaceDeclaration(declaration) => {
                let Some(scoping) = scoping else {
                    return;
                };
                if let Some(namespace) =
                    ConstEnumNamespaceCollector::new(declaration, scoping, include_ambient_enums)
                        .collect()
                {
                    exports.namespaces.insert(namespace.name, namespace.enums);
                }
            }
            _ => {}
        }
    }

    fn analyze_reexport_source(
        &self,
        specifier: &str,
        visited: &mut FxHashSet<PathBuf>,
    ) -> Option<ConstEnumExports> {
        let sources = self.sources.as_ref()?;
        let target = sources.resolve_static_with_declarations(&self.source.path, specifier)?;
        if let Some(cache) = self.cache {
            return cache.analyze_with_visited(target, visited);
        }
        ConstEnumExportAnalyzer::new(target)
            .with_sources(sources)
            .analyze_with_visited(visited)
    }
}

impl ConstEnumExports {
    fn merge(&mut self, other: ConstEnumExports) {
        self.enums.extend(other.enums);
        self.namespaces.extend(other.namespaces);
    }
}

struct ConstEnumNamespace {
    name: String,
    enums: FxHashMap<String, ImportedEnum>,
}

struct ConstEnumNamespaceCollector<'a, 'b> {
    declaration: &'a TSNamespaceDeclaration<'a>,
    scoping: &'b oxc_semantic::Scoping,
    include_ambient_enums: bool,
}

impl<'a, 'b> ConstEnumNamespaceCollector<'a, 'b> {
    fn new(
        declaration: &'a TSNamespaceDeclaration<'a>,
        scoping: &'b oxc_semantic::Scoping,
        include_ambient_enums: bool,
    ) -> Self {
        Self {
            declaration,
            scoping,
            include_ambient_enums,
        }
    }

    fn collect(&self) -> Option<ConstEnumNamespace> {
        let name = &self.declaration.id;
        let TSNamespaceDeclarationBody::TSModuleBlock(block) = &self.declaration.body else {
            return None;
        };
        let mut enums = FxHashMap::default();
        for statement in &block.body {
            let enum_decl = match statement {
                Statement::TSEnumDeclaration(enum_decl) => Some(enum_decl.as_ref()),
                Statement::ExportDeclaration(export) => match &export.declaration {
                    Declaration::TSEnumDeclaration(enum_decl) => Some(enum_decl.as_ref()),
                    _ => None,
                },
                _ => None,
            };
            let Some(enum_decl) = enum_decl else {
                continue;
            };
            if enum_decl.r#const || self.include_ambient_enums {
                if let Some(members) =
                    ConstEnumMemberCollector::new(enum_decl, self.scoping).collect()
                {
                    let imported_enum = if enum_decl.r#const {
                        ImportedEnum::const_enum(members)
                    } else {
                        ImportedEnum::ambient_enum(members)
                    };
                    enums.insert(enum_decl.id.name.to_string(), imported_enum);
                }
            }
        }
        (!enums.is_empty()).then(|| ConstEnumNamespace {
            name: name.name.to_string(),
            enums,
        })
    }
}

struct ConstEnumMemberCollector<'a, 'b> {
    declaration: &'a oxc_ast::ast::TSEnumDeclaration<'a>,
    scoping: &'b oxc_semantic::Scoping,
}

impl<'a, 'b> ConstEnumMemberCollector<'a, 'b> {
    fn new(
        declaration: &'a oxc_ast::ast::TSEnumDeclaration<'a>,
        scoping: &'b oxc_semantic::Scoping,
    ) -> Self {
        Self {
            declaration,
            scoping,
        }
    }

    fn collect(&self) -> Option<FxHashMap<String, EnumValue>> {
        self.collect_from_semantic()
            .or_else(|| ConstEnumAstMemberCollector::new(self.declaration).collect())
    }

    fn collect_from_semantic(&self) -> Option<FxHashMap<String, EnumValue>> {
        let enum_symbol = self.declaration.id.symbol_id();
        let enum_scope = self
            .scoping
            .get_enum_body_scopes(enum_symbol)?
            .first()
            .copied()?;
        let mut members = FxHashMap::default();
        for member in &self.declaration.body.members {
            let member_name = member.id.static_name().to_string();
            let symbol = self
                .scoping
                .get_binding(enum_scope, member_name.as_str().into())?;
            let value = self.scoping.get_enum_member_value(symbol)?;
            let value = match value {
                oxc_syntax::constant_value::ConstantValue::Number(number) => {
                    EnumValue::Number(*number)
                }
                oxc_syntax::constant_value::ConstantValue::String(text) => {
                    EnumValue::String(text.to_string())
                }
            };
            members.insert(member_name, value);
        }
        Some(members)
    }
}

struct ConstEnumAstMemberCollector<'a> {
    declaration: &'a oxc_ast::ast::TSEnumDeclaration<'a>,
}

impl<'a> ConstEnumAstMemberCollector<'a> {
    fn new(declaration: &'a oxc_ast::ast::TSEnumDeclaration<'a>) -> Self {
        Self { declaration }
    }

    fn collect(&self) -> Option<FxHashMap<String, EnumValue>> {
        let mut members = FxHashMap::default();
        let mut previous_number = None;
        for member in &self.declaration.body.members {
            let member_name = member.id.static_name().to_string();
            let value = if let Some(initializer) = &member.initializer {
                self.literal_value(initializer)?
            } else {
                EnumValue::Number(previous_number.map_or(0.0, |number| number + 1.0))
            };
            previous_number = match value {
                EnumValue::Number(number) => Some(number),
                EnumValue::String(_) => None,
            };
            members.insert(member_name, value);
        }
        Some(members)
    }

    fn literal_value(&self, expression: &Expression<'_>) -> Option<EnumValue> {
        match expression {
            Expression::NumericLiteral(literal) => Some(EnumValue::Number(literal.value)),
            Expression::StringLiteral(literal) => {
                Some(EnumValue::String(literal.value.to_string()))
            }
            Expression::UnaryExpression(unary) => {
                let Expression::NumericLiteral(literal) = &unary.argument else {
                    return None;
                };
                match unary.operator {
                    UnaryOperator::UnaryPlus => Some(EnumValue::Number(literal.value)),
                    UnaryOperator::UnaryNegation => Some(EnumValue::Number(-literal.value)),
                    _ => None,
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.literal_value(&parenthesized.expression)
            }
            _ => None,
        }
    }
}

struct ConstEnumInlineMutator<'a> {
    ast: AstBuilder<'a>,
    imports: FxHashMap<String, ConstEnumImport>,
    in_enum_initializer: bool,
    changed: bool,
    inlined_import_locals: FxHashSet<String>,
}

impl<'a> VisitMut<'a> for ConstEnumInlineMutator<'a> {
    fn visit_expression(&mut self, expr: &mut Expression<'a>) {
        if let Some(replacement) = self.const_enum_replacement(expr).cloned() {
            if let Some(local_name) = self.inlined_import_local_name(expr) {
                self.inlined_import_locals.insert(local_name.to_string());
            }
            *expr = ConstEnumValueEmitter::new(&replacement).expression(&self.ast);
            self.changed = true;
            return;
        }
        walk_mut::walk_expression(self, expr);
    }

    fn visit_ts_enum_member(&mut self, member: &mut oxc_ast::ast::TSEnumMember<'a>) {
        let previous = self.in_enum_initializer;
        self.in_enum_initializer = true;
        walk_mut::walk_ts_enum_member(self, member);
        self.in_enum_initializer = previous;
    }
}

impl ConstEnumInlineMutator<'_> {
    fn inlined_import_local_name<'a>(&self, expr: &'a Expression<'a>) -> Option<&'a str> {
        let Expression::StaticMemberExpression(member) = expr else {
            return None;
        };
        match &member.object {
            Expression::Identifier(identifier) => Some(identifier.name.as_str()),
            Expression::StaticMemberExpression(namespace_member) => {
                let Expression::Identifier(identifier) = &namespace_member.object else {
                    return None;
                };
                Some(identifier.name.as_str())
            }
            _ => None,
        }
    }

    fn const_enum_replacement<'a>(&'a self, expr: &Expression<'_>) -> Option<&'a EnumValue> {
        let Expression::StaticMemberExpression(member) = expr else {
            return None;
        };
        let member_name = member.property.name.as_str();

        if let Expression::Identifier(object) = &member.object {
            if let Some(ConstEnumImport::Enum(imported_enum)) =
                self.imports.get(object.name.as_str())
            {
                return self
                    .should_inline(imported_enum)
                    .then(|| imported_enum.members.get(member_name))
                    .flatten();
            }
        }

        let Expression::StaticMemberExpression(namespace_member) = &member.object else {
            return None;
        };
        let Expression::Identifier(namespace) = &namespace_member.object else {
            return None;
        };
        let enum_name = namespace_member.property.name.as_str();
        let ConstEnumImport::Namespace(enums) = self.imports.get(namespace.name.as_str())? else {
            return None;
        };
        let imported_enum = enums.get(enum_name)?;
        self.should_inline(imported_enum)
            .then(|| imported_enum.members.get(member_name))
            .flatten()
    }

    fn should_inline(&self, imported_enum: &ImportedEnum) -> bool {
        imported_enum.inline_everywhere || self.in_enum_initializer
    }
}

struct ConstEnumValueEmitter<'a> {
    value: &'a EnumValue,
}

impl<'a> ConstEnumValueEmitter<'a> {
    fn new(value: &'a EnumValue) -> Self {
        Self { value }
    }

    fn expression<'b>(&self, ast: &AstBuilder<'b>) -> Expression<'b> {
        match self.value {
            EnumValue::Number(number) => Expression::new_numeric_literal(
                SPAN,
                *number,
                None,
                if number.fract() == 0.0 {
                    NumberBase::Decimal
                } else {
                    NumberBase::Float
                },
                ast,
            ),
            EnumValue::String(text) => {
                Expression::new_string_literal(SPAN, Str::from_str_in(text, ast), None, ast)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn snapshot_file(path: &Path, source: &str) -> TransformSourceSnapshot {
        let mut files = FxHashMap::default();
        files.insert(arkdown_path::lexical_normalize(path), Arc::from(source));
        TransformSourceSnapshot::new(files)
    }

    #[test]
    fn typescript_directive_flags_use_comment_ast_not_string_matches() {
        assert!(!TypeScriptDirectiveFlags::from_source(
            "const marker = '// @ts-ignore';\n// ordinary comment",
            "Index.ts",
        )
        .should_emit_javascript());
        assert!(
            TypeScriptDirectiveFlags::from_source(
                "// @ts-ignore: runtime declaration is intentionally untyped\nconst value: string = runtime.value;",
                "Index.ts",
            )
            .should_emit_javascript()
        );
        assert!(TypeScriptDirectiveFlags::from_source(
            "//@ts-noCheck\nconst value: string = runtime.value;",
            "ResourceTable.ts",
        )
        .should_emit_javascript());
        assert!(TypeScriptDirectiveFlags::from_source(
            "// @TS-IGNORE\nconst value: string = runtime.value;",
            "Index.ts",
        )
        .should_emit_javascript());
        assert!(!TypeScriptDirectiveFlags::from_source(
            "// @keepTs\n// @ts-ignore\nconst value: string = runtime.value;",
            "Index.ts",
        )
        .should_emit_javascript());
    }

    #[test]
    fn shared_module_directive_matches_sdk_first_non_import_statement_contract() {
        assert!(TypeScriptDirectiveFlags::from_source(
            "import collections from '@ohos:arkts.collections';\nimport emitter from '@ohos:events.emitter';\n\"use shared\";\nexport class ResourceWatcher {}",
            "ResourceWatcher.ts",
        )
        .is_shared_module());
        assert!(!TypeScriptDirectiveFlags::from_source(
            "const marker = 'not an import';\n\"use shared\";",
            "NotShared.ts",
        )
        .is_shared_module());
        assert!(!TypeScriptDirectiveFlags::from_source(
            "import value from './value';\nconst marker = \"use shared\";",
            "StringValue.ts",
        )
        .is_shared_module());
    }

    #[test]
    fn emits_js_and_inlines_imported_const_enum() {
        let dir = tempfile::tempdir().unwrap();
        let enum_path = dir.path().join("ErrorCode.ts");
        let enum_source = "export const enum ErrorCode { OK, INTERNAL_ERROR = 1240001 }\n";
        let sources = snapshot_file(&enum_path, enum_source);
        let source_path = dir.path().join("Use.ts");
        let source = "import { ErrorCode } from './ErrorCode';\nconst x: ErrorCode = ErrorCode.INTERNAL_ERROR;\n";
        let cache = ConstEnumExportCache::new(sources.clone());
        let output =
            TypeScriptJavascriptEmitter::new(source, source_path.to_str().unwrap(), &sources)
                .with_const_enum_export_cache(&cache)
                .emit_with_source_map()
                .map(|output| output.code)
                .unwrap();
        assert!(
            output.contains("1240001") && !output.contains("from './ErrorCode'"),
            "const enum import must be inlined and removed; got:\n{output}"
        );
        assert!(
            !output.contains(": ErrorCode"),
            "Type annotations must be stripped; got:\n{output}"
        );
    }

    #[test]
    fn mirrors_hvigor_local_enum_value_substitution_with_symbol_identity() {
        let source = r#"
export const enum ConditionType {
    EqualTo = 'EqualTo',
    Limit = 'Limit',
}
export enum Sequence {
    First,
    Second,
    Third = Second + 1,
}
const direct = ConditionType.EqualTo;
const computed = ConditionType['Limit'];
const runtimeEnumValue = Sequence.Second;
function shadow(ConditionType: { EqualTo: string }) {
    return ConditionType.EqualTo;
}
"#;
        let allocator = Allocator::default();
        let mut parsed = Parser::new(&allocator, source, SourceType::ts()).parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let semantic = SemanticBuilder::new()
            .with_enum_eval(true)
            .build(&parsed.program);
        assert!(
            semantic.diagnostics.is_empty(),
            "{:?}",
            semantic.diagnostics
        );
        let scoping = semantic.semantic.into_scoping();

        let changed =
            TypeScriptEnumValueRewriter::rewrite_program(&allocator, &mut parsed.program, &scoping);
        let output = Codegen::new().build(&parsed.program).code;

        assert!(changed, "local const enum members must be substituted");
        assert!(
            output.contains("export const enum ConditionType"),
            "the SDK retains the enum declaration in CompileArkTS source:\n{output}"
        );
        assert!(
            output.contains("const direct = \"EqualTo\";")
                && output.contains("const computed = \"Limit\";"),
            "property and element const-enum access must become literals:\n{output}"
        );
        assert!(
            output.contains("return ConditionType.EqualTo;"),
            "a shadowing parameter must not be mistaken for the enum:\n{output}"
        );
        assert!(
            output.contains("First = 0")
                && output.contains("Second = 1")
                && output.contains("Third = 2"),
            "the SDK emit resolver normalizes every local enum member initializer:\n{output}"
        );
        assert!(
            output.contains("const runtimeEnumValue = Sequence.Second;"),
            "regular enum property access remains a runtime access:\n{output}"
        );
    }

    #[test]
    fn emits_suppressed_typescript_static_fields_as_hvigor_post_class_assignments() {
        let source_path = PathBuf::from("/tmp/project/oh_modules/example/StaticFields.ts");
        let source = r#"// @ts-ignore
export class StaticFields {
  static answer: number = 42;
}
"#;
        let output = TypeScriptJavascriptEmitter::new(
            source,
            source_path.to_str().expect("utf-8 source path"),
            &TransformSourceSnapshot::default(),
        )
        .emit_with_source_map()
        .map(|output| output.code)
        .expect("TypeScript JavaScript emission");

        // Latest ets-loader `ets_checker.js::getTargetESVersion` defaults to
        // ES2021. TypeScript emits this as a post-class assignment, while the
        // former OXC default introduced an ES2022 static block and an extra
        // es2abc function record.
        assert!(
            output.contains("StaticFields.answer = 42"),
            "expected hvigor-style static field assignment:\n{output}"
        );
        assert!(
            !output.contains("static {"),
            "static block changes the SDK ABC function shape:\n{output}"
        );
    }

    #[test]
    fn emits_regular_enums_with_hvigor_var_iife_shape() {
        let source_path = PathBuf::from("/tmp/project/oh_modules/example/QueryType.ts");
        let source = r#"// @ts-ignore
export enum QueryType {
  CONVENTIONAL_QUERY = 0,
  AVG_QUERY
}
"#;
        let output = TypeScriptJavascriptEmitter::new(
            source,
            source_path.to_str().expect("UTF-8 fixture path"),
            &TransformSourceSnapshot::default(),
        )
        .emit_with_source_map()
        .map(|output| output.code)
        .expect("TypeScript JavaScript emission");

        // Latest ets-loader delegates suppression-pragma emission to the
        // TypeScript printer, which uses a hoisted var and an enum IIFE with
        // no return value. OXC's default `let = /* @__PURE__ */ IIFE({})`
        // form changes the generated es2abc function records.
        assert!(
            output.contains("export var QueryType;"),
            "expected SDK enum declaration shape:\n{output}"
        );
        assert!(
            output.contains("})(QueryType || (QueryType = {}));"),
            "expected SDK enum IIFE argument shape:\n{output}"
        );
        assert!(
            !output.contains("return QueryType") && !output.contains("@__PURE__"),
            "OXC-only enum return/pure annotation must be removed:\n{output}"
        );
    }

    #[test]
    fn emits_non_exported_enums_with_hvigor_var_iife_shape() {
        let source_path = PathBuf::from("/tmp/project/entry/src/main/ets/Page.ts");
        let source = "const selected = Option.Buy;\nenum Option { Buy }\n";
        let output = TypeScriptJavascriptEmitter::new(
            source,
            source_path.to_str().expect("UTF-8 fixture path"),
            &TransformSourceSnapshot::default(),
        )
        .emit_with_source_map()
        .map(|output| output.code)
        .expect("TypeScript JavaScript emission");

        assert!(output.contains("var Option;"), "{output}");
        assert!(output.contains("})(Option || (Option = {}));"), "{output}");
        assert!(!output.contains("return Option"), "{output}");
    }

    #[test]
    fn emits_legacy_decorators_with_typescript_runtime_helpers() {
        let source_path = PathBuf::from("/tmp/project/entry/src/main/ets/Value.ts");
        let source = r#"class Value {
  @Local enabled: boolean = false;
}
"#;
        let output = TypeScriptJavascriptEmitter::new(
            source,
            source_path.to_str().expect("UTF-8 fixture path"),
            &TransformSourceSnapshot::default(),
        )
        .emit_with_source_map()
        .map(|output| output.code)
        .expect("TypeScript JavaScript emission");

        assert!(output.contains("var __decorate ="), "{output}");
        assert!(
            output.contains("__decorate([Local], Value.prototype, \"enabled\", void 0)"),
            "{output}"
        );
        assert!(
            !output.contains("@oxc-project/runtime") && !output.contains("import _decorate"),
            "OXC helper imports and aliases are not valid OHOS runtime dependencies:\n{output}"
        );
    }

    #[test]
    fn emits_synthesized_derived_constructor_with_typescript_arguments_forwarding() {
        let source_path = PathBuf::from("/tmp/project/entry/src/main/ets/DataSource.ts");
        let source = r#"class BasicDataSource {}
class MyDataSource extends BasicDataSource {
  private values: string[] = [];
}
"#;
        let output = TypeScriptJavascriptEmitter::new(
            source,
            source_path.to_str().expect("UTF-8 fixture path"),
            &TransformSourceSnapshot::default(),
        )
        .emit_with_source_map()
        .map(|output| output.code)
        .expect("TypeScript JavaScript emission");

        assert!(
            output.contains("constructor()") && output.contains("super(...arguments);"),
            "{output}"
        );
        assert!(
            !output.contains("..._args"),
            "OXC's synthesized forwarding parameter changes hvigor output:\n{output}"
        );
    }

    #[test]
    fn emits_static_self_references_without_oxc_class_aliases() {
        let source_path = PathBuf::from("/tmp/project/oh_modules/example/OpeTypeConversion.ts");
        let source = r#"// @ts-ignore
export class OpeTypeConversion {
  private static readonly NUMBER_RADIX_DECIMAL = 10;
  private static readonly NUMBER_RADIX_DECIMAL_LOG2 = Math.log2(OpeTypeConversion.NUMBER_RADIX_DECIMAL);
}
"#;
        let output = TypeScriptJavascriptEmitter::new(
            source,
            source_path.to_str().expect("UTF-8 fixture path"),
            &TransformSourceSnapshot::default(),
        )
        .emit_with_source_map()
        .map(|output| output.code)
        .expect("TypeScript JavaScript emission");

        // TypeScript emits the post-class static assignment against the class
        // itself. OXC's temporary `_OpeTypeConversion` binding adds an extra
        // module initializer and changes the ABC main function.
        assert!(
            output.contains(
                "OpeTypeConversion.NUMBER_RADIX_DECIMAL_LOG2 = Math.log2(OpeTypeConversion.NUMBER_RADIX_DECIMAL);"
            ),
            "expected SDK static self-reference shape:\n{output}"
        );
        assert!(
            !output.contains("_OpeTypeConversion"),
            "OXC-only class alias must be absent:\n{output}"
        );
    }

    #[test]
    fn inlines_namespace_const_enum() {
        let dir = tempfile::tempdir().unwrap();
        let enum_path = dir.path().join("Constant.ts");
        let enum_source =
            "export namespace Constant { export const enum Key { MapProviderKey = 'provider' } }\n";
        let sources = snapshot_file(&enum_path, enum_source);
        let source_path = dir.path().join("Use.ts");
        let source =
            "import { Constant } from './Constant';\nconst key = Constant.Key.MapProviderKey;\n";
        let output =
            TypeScriptJavascriptEmitter::new(source, source_path.to_str().unwrap(), &sources)
                .emit_with_source_map()
                .map(|output| output.code)
                .unwrap();
        assert!(
            output.contains("\"provider\"") && !output.contains("Constant.Key.MapProviderKey"),
            "namespace const enum must be inlined; got:\n{output}"
        );
    }

    #[test]
    fn folds_sdk_ambient_enum_only_inside_enum_initializers_like_hvigor() {
        let sdk_root = PathBuf::from("/sdk/ets/api");
        let kit_root = PathBuf::from("/sdk/ets/kits");
        let declaration = sdk_root.join("@ohos.multimedia.media.d.ts");
        let mut files = FxHashMap::default();
        files.insert(
            declaration,
            Arc::<str>::from(
                r#"declare namespace media {
    declare interface OutputSize {}
    enum VideoScaleType {
        VIDEO_SCALE_TYPE_FIT = 0,
        VIDEO_SCALE_TYPE_FIT_CROP = 1
    }
}
export default media;"#,
            ),
        );
        files.insert(
            kit_root.join("@kit.MediaKit.d.ts"),
            Arc::<str>::from(
                r#"import media from "@ohos.multimedia.media";
export { media };"#,
            ),
        );
        let sources = TransformSourceSnapshot::new(files)
            .with_sdk_declaration_roots(vec![sdk_root, kit_root]);
        let source_path = "/project/PlayerInterface.ets";
        let source = r#"import { media } from "@kit.MediaKit";
export enum MLiveVideoScaleMode {
    FIT = media.VideoScaleType.VIDEO_SCALE_TYPE_FIT,
    CROP = media.VideoScaleType.VIDEO_SCALE_TYPE_FIT_CROP
}
const runtimeScale = media.VideoScaleType.VIDEO_SCALE_TYPE_FIT;"#;
        let allocator = Allocator::default();
        let mut parsed = Parser::new(&allocator, source, SourceType::ets()).parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);

        let changed = ImportedConstEnumRewriter::new(source_path, &sources)
            .rewrite_program(&allocator, &mut parsed.program);
        let output = Codegen::new().build(&parsed.program).code;

        assert!(changed, "SDK ambient enum initializer must be folded");
        assert!(
            output.contains("FIT = 0") && output.contains("CROP = 1"),
            "hvigor folds imported SDK enum constants in enum members:\n{output}"
        );
        assert!(
            output.contains("const runtimeScale = media.VideoScaleType.VIDEO_SCALE_TYPE_FIT;"),
            "a non-const ambient enum access outside an enum initializer stays live:\n{output}"
        );
    }

    #[test]
    fn emit_js_matches_typescript_class_field_semantics() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("Model.ts");
        let source =
            "class Model { value: string; count = 1; constructor() { this.value = 'ok'; } }\n";
        let sources = TransformSourceSnapshot::default();
        let output =
            TypeScriptJavascriptEmitter::new(source, source_path.to_str().unwrap(), &sources)
                .emit_with_source_map()
                .map(|output| output.code)
                .unwrap();
        assert!(
            !output.contains("value;"),
            "type-only class fields must be removed like TypeScript useDefineForClassFields=false; got:\n{output}"
        );
        assert!(
            output.contains("this.count = 1"),
            "initialized class fields must be lowered into constructor assignments; got:\n{output}"
        );
    }

    #[test]
    fn inlines_const_enum_reexported_from_package_native_declaration() {
        let widget_root = PathBuf::from("/project/oh_modules/@scope/widget");
        let widget_index = widget_root.join("Index.ets");
        let native_root = widget_root.join("native-declarations/custom-location");
        let native_index = native_root.join("Entry.d.ts");
        let native_types = native_root.join("Types.d.ts");

        let mut files = FxHashMap::default();
        files.insert(
            widget_index.clone(),
            Arc::<str>::from(r#"export { EdgeType } from "libwidget.so";"#),
        );
        files.insert(
            native_index.clone(),
            Arc::<str>::from(r#"export * from "./Types";"#),
        );
        files.insert(
            native_types,
            Arc::<str>::from(r#"export const enum EdgeType { TOP = 1, BOTTOM = 3 }"#),
        );
        let sources = TransformSourceSnapshot::with_resolved_static_modules(
            files,
            vec![crate::source::TransformResolvedStaticModule::new(
                &widget_root,
                "libwidget.so",
                &native_index,
            )],
        );
        let source_path = PathBuf::from("/project/Use.ts");
        let source = r#"import { EdgeType } from "@scope/widget";
const top = EdgeType.TOP;"#;
        let resolved_package = sources
            .resolve_static_with_declarations(&source_path, "@scope/widget")
            .expect("package entry resolves");
        assert_eq!(resolved_package.path, widget_index);
        let resolved_native = sources
            .resolve_static_with_declarations(&resolved_package.path, "libwidget.so")
            .expect("native declaration resolves");
        assert_eq!(resolved_native.path, native_index);
        let native_exports = ConstEnumExportAnalyzer::new(resolved_native)
            .with_sources(&sources)
            .analyze()
            .expect("native const enum exports");
        assert!(
            native_exports.enums.contains_key("EdgeType"),
            "native declaration export-all must expose EdgeType const enum"
        );
        let exports = ConstEnumExportAnalyzer::new(resolved_package)
            .with_sources(&sources)
            .analyze()
            .expect("const enum exports");
        assert!(
            exports.enums.contains_key("EdgeType"),
            "package native reexport must expose EdgeType const enum"
        );

        let output =
            TypeScriptJavascriptEmitter::new(source, source_path.to_str().unwrap(), &sources)
                .emit_with_source_map()
                .map(|output| output.code)
                .unwrap();

        assert!(
            output.contains("const top = 1"),
            "native declaration const enum must be inlined; got:\n{output}"
        );
        assert!(
            !output.contains("@scope/widget") && !output.contains("EdgeType.TOP"),
            "const enum import must be erased after inlining; got:\n{output}"
        );
    }
}
