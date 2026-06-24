//! F20 — BjcCov-style coverage instrumentation for JS chunks.
//!
//! Instruments JavaScript output chunks with coverage counters matching the
//! format emitted by hvigor's `@ohos/coverage` plugin (bjc format), so that
//! the ArkTS runtime's coverage reporter can track which code paths execute.
//!
//! hvigor injects a `BjcCov` class at file start and then calls:
//! - `bcvHASH.instrumentFunction(funcIndex)` — at each function entry
//! - `bcvHASH.instrumentRegion(funcIndex, regionIndex)` — for statement regions
//! - `bcvHASH.instrumentBranch(funcIndex, branchIndex, trueOrFalse)` — at branches
//!
//! Upstream ref: `@ohos/coverage/res/template/bjc_cov_class.ts` in the OHOS SDK.

use arkdown_cache::CacheKeyMaterial;
use arkdown_path::ArtifactPathSerializer;
use oxc_allocator::{Allocator, TakeIn, Vec as OxcVec};
use oxc_ast::ast::{
  Argument, ArrayExpressionElement, ArrowFunctionExpression, AssignmentTarget, ClassElement,
  ClassType, ConditionalExpression, DoWhileStatement, Expression, ForInStatement, ForOfStatement,
  ForStatement, FormalParameter, FormalParameterKind, FormalParameters, Function, FunctionType,
  IfStatement, MemberExpression, MethodDefinitionKind, MethodDefinitionType, NumberBase,
  ObjectPropertyKind, PropertyDefinitionType, PropertyKey, PropertyKind, SimpleAssignmentTarget,
  Statement, SwitchStatement, TryStatement, VariableDeclarationKind, WhileStatement,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_span::{SourceType, Span, SPAN};
use oxc_syntax::operator::{AssignmentOperator, LogicalOperator, UnaryOperator, UpdateOperator};

struct BjcCovRuntimeName;

impl BjcCovRuntimeName {
  const CLASS: &'static str = "BjcCov";
  const COVERAGE: &'static str = "coverage";
  const COV_DATA: &'static str = "covData";
  const GCV: &'static str = "gcv";
  const GLOBAL_THIS: &'static str = "globalThis";
  const GLOBAL_COVERAGE: &'static str = "__coverage__";
  const CONSTRUCTOR: &'static str = "constructor";
  const FUNC: &'static str = "func";
  const REGION: &'static str = "region";
  const BRANCH: &'static str = "branch";
  const TRUE_OR_FALSE: &'static str = "trueOrFalse";
  const PATH: &'static str = "path";
  const FUNCTIONS: &'static str = "functions";
  const NAME: &'static str = "name";
  const COUNT: &'static str = "count";
  const REGIONS: &'static str = "regions";
  const BRANCHES: &'static str = "branches";
  const TRUE_COUNT: &'static str = "trueCount";
  const FALSE_COUNT: &'static str = "falseCount";
  const INSTRUMENT_FUNCTION: &'static str = "instrumentFunction";
  const INSTRUMENT_REGION: &'static str = "instrumentRegion";
  const INSTRUMENT_BRANCH: &'static str = "instrumentBranch";
}

struct BjcCovRuntimeEmitter<'a> {
  ast: AstBuilder<'a>,
}

impl<'a> BjcCovRuntimeEmitter<'a> {
  fn new(allocator: &'a Allocator) -> Self {
    Self {
      ast: AstBuilder::new(allocator),
    }
  }

  fn statements(&self) -> OxcVec<'a, Statement<'a>> {
    self.ast.vec1(self.class_statement())
  }

  fn class_statement(&self) -> Statement<'a> {
    let body = self.ast.alloc_class_body(
      SPAN,
      self.ast.vec_from_iter([
        self.coverage_property(),
        self.constructor_method(),
        self.instrument_function_method(),
        self.instrument_region_method(),
        self.instrument_branch_method(),
      ]),
    );
    let class = self.ast.alloc_class(
      SPAN,
      ClassType::ClassDeclaration,
      self.ast.vec(),
      Some(self.ast.binding_identifier(SPAN, BjcCovRuntimeName::CLASS)),
      NONE,
      None,
      NONE,
      self.ast.vec(),
      body,
      false,
      false,
    );
    Statement::ClassDeclaration(class)
  }

  fn coverage_property(&self) -> ClassElement<'a> {
    self.ast.class_element_property_definition(
      SPAN,
      PropertyDefinitionType::PropertyDefinition,
      self.ast.vec(),
      self.property_key(BjcCovRuntimeName::COVERAGE),
      Option::<oxc_allocator::Box<'a, oxc_ast::ast::TSTypeAnnotation<'a>>>::None,
      None,
      false,
      false,
      false,
      false,
      false,
      false,
      false,
      None,
    )
  }

  fn constructor_method(&self) -> ClassElement<'a> {
    self.method(
      BjcCovRuntimeName::CONSTRUCTOR,
      MethodDefinitionKind::Constructor,
      [BjcCovRuntimeName::COV_DATA],
      self.ast.vec_from_iter([
        self.variable_statement(
          VariableDeclarationKind::Const,
          BjcCovRuntimeName::GCV,
          self
            .ast
            .expression_string_literal(SPAN, BjcCovRuntimeName::GLOBAL_COVERAGE, None),
        ),
        self.variable_statement(
          VariableDeclarationKind::Let,
          BjcCovRuntimeName::COVERAGE,
          self.ast.expression_logical(
            SPAN,
            self.global_coverage_expression(),
            LogicalOperator::Or,
            self.global_coverage_assignment_expression(),
          ),
        ),
        self.coverage_init_statement(),
        self.this_coverage_assignment_statement(),
      ]),
    )
  }

  fn instrument_function_method(&self) -> ClassElement<'a> {
    self.method(
      BjcCovRuntimeName::INSTRUMENT_FUNCTION,
      MethodDefinitionKind::Method,
      [BjcCovRuntimeName::FUNC],
      self.ast.vec_from_iter([
        self.increment_statement(self.function_count_target()),
        self.increment_statement(self.function_first_region_count_target()),
      ]),
    )
  }

  fn instrument_region_method(&self) -> ClassElement<'a> {
    self.method(
      BjcCovRuntimeName::INSTRUMENT_REGION,
      MethodDefinitionKind::Method,
      [BjcCovRuntimeName::FUNC, BjcCovRuntimeName::REGION],
      self
        .ast
        .vec1(self.increment_statement(self.region_count_target())),
    )
  }

  fn instrument_branch_method(&self) -> ClassElement<'a> {
    self.method(
      BjcCovRuntimeName::INSTRUMENT_BRANCH,
      MethodDefinitionKind::Method,
      [
        BjcCovRuntimeName::FUNC,
        BjcCovRuntimeName::BRANCH,
        BjcCovRuntimeName::TRUE_OR_FALSE,
      ],
      self.ast.vec1(
        self.ast.statement_if(
          SPAN,
          self.identifier(BjcCovRuntimeName::TRUE_OR_FALSE),
          self.ast.statement_block(
            SPAN,
            self
              .ast
              .vec1(self.increment_statement(self.branch_count_target(true))),
          ),
          Some(
            self.ast.statement_block(
              SPAN,
              self
                .ast
                .vec1(self.increment_statement(self.branch_count_target(false))),
            ),
          ),
        ),
      ),
    )
  }

  fn method<const N: usize>(
    &self,
    name: &'a str,
    kind: MethodDefinitionKind,
    params: [&'a str; N],
    statements: OxcVec<'a, Statement<'a>>,
  ) -> ClassElement<'a> {
    let function = self.ast.alloc_function(
      SPAN,
      FunctionType::FunctionExpression,
      None,
      false,
      false,
      false,
      NONE,
      NONE,
      self.formal_parameters(params),
      Option::<oxc_allocator::Box<'a, oxc_ast::ast::TSTypeAnnotation<'a>>>::None,
      Some(self.ast.function_body(SPAN, self.ast.vec(), statements)),
    );
    self.ast.class_element_method_definition(
      SPAN,
      MethodDefinitionType::MethodDefinition,
      self.ast.vec(),
      self.property_key(name),
      function,
      kind,
      false,
      false,
      false,
      false,
      None,
    )
  }

  fn formal_parameters<const N: usize>(
    &self,
    params: [&'a str; N],
  ) -> oxc_allocator::Box<'a, FormalParameters<'a>> {
    self.ast.alloc_formal_parameters(
      SPAN,
      FormalParameterKind::FormalParameter,
      self
        .ast
        .vec_from_iter(params.into_iter().map(|name| self.formal_parameter(name))),
      NONE,
    )
  }

  fn formal_parameter(&self, name: &'a str) -> FormalParameter<'a> {
    self.ast.formal_parameter(
      SPAN,
      self.ast.vec(),
      self.ast.binding_pattern_binding_identifier(SPAN, name),
      NONE,
      NONE,
      false,
      None,
      false,
      false,
    )
  }

  fn variable_statement(
    &self,
    kind: VariableDeclarationKind,
    name: &'a str,
    init: Expression<'a>,
  ) -> Statement<'a> {
    Statement::from(self.ast.declaration_variable(
      SPAN,
      kind,
      self.ast.vec1(self.ast.variable_declarator(
        SPAN,
        kind,
        self.ast.binding_pattern_binding_identifier(SPAN, name),
        NONE,
        Some(init),
        false,
      )),
      false,
    ))
  }

  fn coverage_init_statement(&self) -> Statement<'a> {
    self.ast.statement_if(
      SPAN,
      self.ast.expression_unary(
        SPAN,
        UnaryOperator::LogicalNot,
        self.coverage_entry_expression(),
      ),
      self.ast.statement_block(
        SPAN,
        self
          .ast
          .vec1(self.assignment_statement(self.coverage_entry_target(), self.cov_data())),
      ),
      None,
    )
  }

  fn this_coverage_assignment_statement(&self) -> Statement<'a> {
    self.assignment_statement(
      self.this_coverage_target(),
      self.coverage_entry_expression(),
    )
  }

  fn assignment_statement(
    &self,
    left: AssignmentTarget<'a>,
    right: Expression<'a>,
  ) -> Statement<'a> {
    self.ast.statement_expression(
      SPAN,
      self
        .ast
        .expression_assignment(SPAN, AssignmentOperator::Assign, left, right),
    )
  }

  fn global_coverage_assignment_expression(&self) -> Expression<'a> {
    self.ast.expression_assignment(
      SPAN,
      AssignmentOperator::Assign,
      self.global_coverage_target(),
      self.ast.expression_object(SPAN, self.ast.vec()),
    )
  }

  fn increment_statement(&self, target: SimpleAssignmentTarget<'a>) -> Statement<'a> {
    self.ast.statement_expression(
      SPAN,
      self
        .ast
        .expression_update(SPAN, UpdateOperator::Increment, false, target),
    )
  }

  fn function_count_target(&self) -> SimpleAssignmentTarget<'a> {
    self.static_assignment_target(
      self.function_record_expression(BjcCovRuntimeName::FUNC),
      BjcCovRuntimeName::COUNT,
    )
  }

  fn function_first_region_count_target(&self) -> SimpleAssignmentTarget<'a> {
    self.static_assignment_target(
      self.computed_member(
        self.static_member(
          self.function_record_expression(BjcCovRuntimeName::FUNC),
          BjcCovRuntimeName::REGIONS,
        ),
        self
          .ast
          .expression_numeric_literal(SPAN, 0.0, Some("0".into()), NumberBase::Decimal),
      ),
      BjcCovRuntimeName::COUNT,
    )
  }

  fn region_count_target(&self) -> SimpleAssignmentTarget<'a> {
    self.static_assignment_target(
      self.computed_member(
        self.static_member(
          self.function_record_expression(BjcCovRuntimeName::FUNC),
          BjcCovRuntimeName::REGIONS,
        ),
        self.identifier(BjcCovRuntimeName::REGION),
      ),
      BjcCovRuntimeName::COUNT,
    )
  }

  fn branch_count_target(&self, true_count: bool) -> SimpleAssignmentTarget<'a> {
    let count_name = if true_count {
      BjcCovRuntimeName::TRUE_COUNT
    } else {
      BjcCovRuntimeName::FALSE_COUNT
    };
    self.static_assignment_target(
      self.computed_member(
        self.static_member(
          self.function_record_expression(BjcCovRuntimeName::FUNC),
          BjcCovRuntimeName::BRANCHES,
        ),
        self.identifier(BjcCovRuntimeName::BRANCH),
      ),
      count_name,
    )
  }

  fn function_record_expression(&self, func_name: &str) -> Expression<'a> {
    self.computed_member(
      self.static_member(
        self.this_coverage_expression(),
        BjcCovRuntimeName::FUNCTIONS,
      ),
      self.identifier(func_name),
    )
  }

  fn global_coverage_expression(&self) -> Expression<'a> {
    self.computed_member(
      self.identifier(BjcCovRuntimeName::GLOBAL_THIS),
      self.identifier(BjcCovRuntimeName::GCV),
    )
  }

  fn coverage_entry_expression(&self) -> Expression<'a> {
    self.computed_member(
      self.identifier(BjcCovRuntimeName::COVERAGE),
      self.cov_data_path_expression(),
    )
  }

  fn cov_data_path_expression(&self) -> Expression<'a> {
    self.static_member(self.cov_data(), BjcCovRuntimeName::PATH)
  }

  fn this_coverage_expression(&self) -> Expression<'a> {
    self.static_member(self.ast.expression_this(SPAN), BjcCovRuntimeName::COVERAGE)
  }

  fn cov_data(&self) -> Expression<'a> {
    self.identifier(BjcCovRuntimeName::COV_DATA)
  }

  fn identifier(&self, name: &str) -> Expression<'a> {
    self.ast.expression_identifier(SPAN, self.ast.str(name))
  }

  fn this_coverage_target(&self) -> AssignmentTarget<'a> {
    AssignmentTarget::from(self.ast.member_expression_static(
      SPAN,
      self.ast.expression_this(SPAN),
      self.ast.identifier_name(SPAN, BjcCovRuntimeName::COVERAGE),
      false,
    ))
  }

  fn global_coverage_target(&self) -> AssignmentTarget<'a> {
    AssignmentTarget::ComputedMemberExpression(self.ast.alloc_computed_member_expression(
      SPAN,
      self.identifier(BjcCovRuntimeName::GLOBAL_THIS),
      self.identifier(BjcCovRuntimeName::GCV),
      false,
    ))
  }

  fn coverage_entry_target(&self) -> AssignmentTarget<'a> {
    AssignmentTarget::ComputedMemberExpression(self.ast.alloc_computed_member_expression(
      SPAN,
      self.identifier(BjcCovRuntimeName::COVERAGE),
      self.cov_data_path_expression(),
      false,
    ))
  }

  fn static_assignment_target(
    &self,
    object: Expression<'a>,
    property: &'a str,
  ) -> SimpleAssignmentTarget<'a> {
    SimpleAssignmentTarget::StaticMemberExpression(self.ast.alloc_static_member_expression(
      SPAN,
      object,
      self.ast.identifier_name(SPAN, property),
      false,
    ))
  }

  fn static_member(&self, object: Expression<'a>, property: &'a str) -> Expression<'a> {
    Expression::from(self.ast.member_expression_static(
      SPAN,
      object,
      self.ast.identifier_name(SPAN, property),
      false,
    ))
  }

  fn computed_member(&self, object: Expression<'a>, expression: Expression<'a>) -> Expression<'a> {
    Expression::ComputedMemberExpression(
      self
        .ast
        .alloc_computed_member_expression(SPAN, object, expression, false),
    )
  }

  fn property_key(&self, name: &'a str) -> PropertyKey<'a> {
    PropertyKey::StaticIdentifier(self.ast.alloc(self.ast.identifier_name(SPAN, name)))
  }
}

pub(super) struct CoverageInstrumenter<'a> {
  source: &'a str,
  file_id: &'a str,
}

impl<'a> CoverageInstrumenter<'a> {
  pub(super) fn new(source: &'a str, file_id: &'a str) -> Self {
    Self { source, file_id }
  }

  /// Instrument a JS chunk with BjcCov-style coverage counters.
  ///
  /// Returns the instrumented code, or the original source unchanged if
  /// instrumentation fails (instrumentation is best-effort; build must not fail).
  pub(super) fn instrument(&self) -> String {
    match self.try_instrument() {
      Ok(instrumented) => instrumented,
      Err(e) => {
        tracing::warn!(
            file_id = self.file_id,
            error = %e,
            "coverage instrumentation failed, using original source"
        );
        self.source.to_string()
      }
    }
  }

  fn try_instrument(&self) -> Result<String, String> {
    let generated_names = CoverageGeneratedNames::new(self.file_id);

    let normalized_path =
      ArtifactPathSerializer::path_to_slash_string(std::path::Path::new(self.file_id))
        .unwrap_or_else(|_| self.file_id.to_string());

    // Parse the source with OXC to collect function info.
    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let mut parsed = Parser::new(&allocator, self.source, source_type).parse();

    if !parsed.diagnostics.is_empty() {
      let msg = parsed
        .diagnostics
        .iter()
        .map(|e| e.message.as_ref())
        .collect::<Vec<_>>()
        .join("; ");
      return Err(format!("OXC parse errors: {msg}"));
    }
    // Walk the AST to collect all function-like nodes.
    let mut collector = FunctionCollector {
      functions: Vec::new(),
      anon_counter: 0,
      func_stack: Vec::new(),
      _marker: std::marker::PhantomData,
    };
    collector.visit_program(&parsed.program);
    let functions = collector.functions;

    CoverageAstInstrumenter::new(&allocator, generated_names.bcv_var())
      .visit_program(&mut parsed.program);

    let ast = AstBuilder::new(&allocator);
    let mut body = OxcVec::new_in(&allocator);
    body.extend(BjcCovRuntimeEmitter::new(&allocator).statements());
    body.extend(
      CoveragePreludeEmitter::new(
        generated_names.cov_data_var(),
        generated_names.bcv_var(),
        &normalized_path,
      )
      .statements(ast, &functions),
    );
    body.extend(parsed.program.body.take_in(ast));
    parsed.program.body = body;

    Ok(Codegen::new().build(&parsed.program).code)
  }
}

struct CoverageGeneratedNames {
  bcv_var: String,
  cov_data_var: String,
}

impl CoverageGeneratedNames {
  fn new(file_id: &str) -> Self {
    let hash = CoverageHashPrefix::new(CacheKeyMaterial::hash_u64(file_id));
    Self {
      bcv_var: CoverageIdentifier::from_parts(["bcv", hash.as_str()]),
      cov_data_var: CoverageIdentifier::from_parts(["covData_", hash.as_str()]),
    }
  }

  fn bcv_var(&self) -> &str {
    &self.bcv_var
  }

  fn cov_data_var(&self) -> &str {
    &self.cov_data_var
  }
}

struct CoverageHashPrefix {
  value: String,
}

impl CoverageHashPrefix {
  fn new(hash: u64) -> Self {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut bytes = [0_u8; 8];
    let mut len = 0;
    let mut started = false;
    for shift in (0..16).rev() {
      let nibble = ((hash >> (shift * 4)) & 0x0f) as usize;
      if nibble != 0 || started || shift == 0 {
        started = true;
        bytes[len] = HEX[nibble];
        len += 1;
        if len == bytes.len() {
          break;
        }
      }
    }

    Self {
      value: bytes[..len].iter().map(|byte| char::from(*byte)).collect(),
    }
  }

  fn as_str(&self) -> &str {
    &self.value
  }
}

struct CoverageIdentifier;

impl CoverageIdentifier {
  fn from_parts<const N: usize>(parts: [&str; N]) -> String {
    parts.into_iter().collect()
  }
}

struct CoveragePreludeEmitter<'a> {
  cov_data_var: &'a str,
  bcv_var: &'a str,
  normalized_path: &'a str,
}

impl<'a> CoveragePreludeEmitter<'a> {
  fn new(cov_data_var: &'a str, bcv_var: &'a str, normalized_path: &'a str) -> Self {
    Self {
      cov_data_var,
      bcv_var,
      normalized_path,
    }
  }

  fn statements<'b>(
    &self,
    ast: AstBuilder<'b>,
    functions: &[FunctionInfo],
  ) -> OxcVec<'b, Statement<'b>> {
    ast.vec_from_iter([
      self.coverage_data_statement(ast, functions),
      self.bjc_instance_statement(ast),
    ])
  }

  fn coverage_data_statement<'b>(
    &self,
    ast: AstBuilder<'b>,
    functions: &[FunctionInfo],
  ) -> Statement<'b> {
    Statement::from(ast.declaration_variable(
      SPAN,
      VariableDeclarationKind::Const,
      ast.vec1(ast.variable_declarator(
        SPAN,
        VariableDeclarationKind::Const,
        ast.binding_pattern_binding_identifier(SPAN, ast.str(self.cov_data_var)),
        NONE,
        Some(ast.expression_object(
          SPAN,
          ast.vec_from_iter([
            self.property(
              ast,
              BjcCovRuntimeName::PATH,
              ast.expression_string_literal(SPAN, ast.str(self.normalized_path), None),
            ),
            self.property(
              ast,
              BjcCovRuntimeName::FUNCTIONS,
              self.functions_expression(ast, functions),
            ),
          ]),
        )),
        false,
      )),
      false,
    ))
  }

  fn bjc_instance_statement<'b>(&self, ast: AstBuilder<'b>) -> Statement<'b> {
    Statement::from(ast.declaration_variable(
      SPAN,
      VariableDeclarationKind::Let,
      ast.vec1(ast.variable_declarator(
        SPAN,
        VariableDeclarationKind::Let,
        ast.binding_pattern_binding_identifier(SPAN, ast.str(self.bcv_var)),
        NONE,
        Some(ast.expression_new(
          SPAN,
          ast.expression_identifier(SPAN, BjcCovRuntimeName::CLASS),
          NONE,
          ast.vec1(Argument::from(
            ast.expression_identifier(SPAN, ast.str(self.cov_data_var)),
          )),
        )),
        false,
      )),
      false,
    ))
  }

  fn functions_expression<'b>(
    &self,
    ast: AstBuilder<'b>,
    functions: &[FunctionInfo],
  ) -> Expression<'b> {
    ast.expression_array(
      SPAN,
      ast.vec_from_iter(
        functions
          .iter()
          .map(|function| ArrayExpressionElement::from(self.function_expression(ast, function))),
      ),
    )
  }

  fn function_expression<'b>(
    &self,
    ast: AstBuilder<'b>,
    function: &FunctionInfo,
  ) -> Expression<'b> {
    ast.expression_object(
      SPAN,
      ast.vec_from_iter([
        self.property(
          ast,
          BjcCovRuntimeName::NAME,
          ast.expression_string_literal(SPAN, ast.str(&function.name), None),
        ),
        self.property(ast, BjcCovRuntimeName::COUNT, self.zero_expression(ast)),
        self.property(
          ast,
          BjcCovRuntimeName::REGIONS,
          self.regions_expression(ast, function),
        ),
        self.property(
          ast,
          BjcCovRuntimeName::BRANCHES,
          self.branches_expression(ast, function),
        ),
      ]),
    )
  }

  fn regions_expression<'b>(&self, ast: AstBuilder<'b>, function: &FunctionInfo) -> Expression<'b> {
    ast.expression_array(
      SPAN,
      ast.vec_from_iter(
        (0..=function.regions.len())
          .map(|_| ArrayExpressionElement::from(self.count_object_expression(ast))),
      ),
    )
  }

  fn branches_expression<'b>(
    &self,
    ast: AstBuilder<'b>,
    function: &FunctionInfo,
  ) -> Expression<'b> {
    ast.expression_array(
      SPAN,
      ast.vec_from_iter(
        function
          .branches
          .iter()
          .map(|_| ArrayExpressionElement::from(self.branch_object_expression(ast))),
      ),
    )
  }

  fn count_object_expression<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
    ast.expression_object(
      SPAN,
      ast.vec1(self.property(ast, BjcCovRuntimeName::COUNT, self.zero_expression(ast))),
    )
  }

  fn branch_object_expression<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
    ast.expression_object(
      SPAN,
      ast.vec_from_iter([
        self.property(
          ast,
          BjcCovRuntimeName::TRUE_COUNT,
          self.zero_expression(ast),
        ),
        self.property(
          ast,
          BjcCovRuntimeName::FALSE_COUNT,
          self.zero_expression(ast),
        ),
      ]),
    )
  }

  fn zero_expression<'b>(&self, ast: AstBuilder<'b>) -> Expression<'b> {
    ast.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal)
  }

  fn property<'b>(
    &self,
    ast: AstBuilder<'b>,
    name: &str,
    value: Expression<'b>,
  ) -> ObjectPropertyKind<'b> {
    ast.object_property_kind_object_property(
      SPAN,
      PropertyKind::Init,
      ast.property_key_static_identifier(SPAN, ast.str(name)),
      value,
      false,
      false,
      false,
    )
  }
}

struct CoverageCallEmitter<'a> {
  bcv_var: &'a str,
}

impl<'a> CoverageCallEmitter<'a> {
  fn new(bcv_var: &'a str) -> Self {
    Self { bcv_var }
  }

  fn function_entry_statement<'b>(
    &self,
    ast: AstBuilder<'b>,
    function_index: usize,
  ) -> Statement<'b> {
    self.call_statement(
      ast,
      BjcCovRuntimeName::INSTRUMENT_FUNCTION,
      [self.index_argument(ast, function_index)],
    )
  }

  fn region_statement<'b>(
    &self,
    ast: AstBuilder<'b>,
    function_index: usize,
    region_index: usize,
  ) -> Statement<'b> {
    self.call_statement(
      ast,
      BjcCovRuntimeName::INSTRUMENT_REGION,
      [
        self.index_argument(ast, function_index),
        self.index_argument(ast, region_index),
      ],
    )
  }

  fn branch_statement<'b>(
    &self,
    ast: AstBuilder<'b>,
    function_index: usize,
    branch_index: usize,
    true_or_false: bool,
  ) -> Statement<'b> {
    self.call_statement(
      ast,
      BjcCovRuntimeName::INSTRUMENT_BRANCH,
      [
        self.index_argument(ast, function_index),
        self.index_argument(ast, branch_index),
        Argument::from(ast.expression_boolean_literal(SPAN, true_or_false)),
      ],
    )
  }

  fn call_statement<'b, const N: usize>(
    &self,
    ast: AstBuilder<'b>,
    method: &str,
    arguments: [Argument<'b>; N],
  ) -> Statement<'b> {
    ast.statement_expression(
      SPAN,
      self.instrument_call_expression(ast, method, ast.vec_from_iter(arguments)),
    )
  }

  fn branch_call_expression<'b>(
    &self,
    ast: AstBuilder<'b>,
    function_index: usize,
    branch_index: usize,
    true_or_false: bool,
  ) -> Expression<'b> {
    self.instrument_call_expression(
      ast,
      BjcCovRuntimeName::INSTRUMENT_BRANCH,
      ast.vec_from_iter([
        self.index_argument(ast, function_index),
        self.index_argument(ast, branch_index),
        Argument::from(ast.expression_boolean_literal(SPAN, true_or_false)),
      ]),
    )
  }

  fn instrument_call_expression<'b>(
    &self,
    ast: AstBuilder<'b>,
    method: &str,
    arguments: oxc_allocator::Vec<'b, Argument<'b>>,
  ) -> Expression<'b> {
    ast.expression_call(
      SPAN,
      Self::member_expression_to_expression(ast.member_expression_static(
        SPAN,
        ast.expression_identifier(SPAN, ast.str(self.bcv_var)),
        ast.identifier_name(SPAN, ast.str(method)),
        false,
      )),
      NONE,
      arguments,
      false,
    )
  }

  fn index_argument<'b>(&self, ast: AstBuilder<'b>, index: usize) -> Argument<'b> {
    Argument::from(ast.expression_numeric_literal(
      SPAN,
      index as f64,
      Some(ast.str(&index.to_string())),
      NumberBase::Decimal,
    ))
  }

  fn member_expression_to_expression<'b>(member: MemberExpression<'b>) -> Expression<'b> {
    match member {
      MemberExpression::ComputedMemberExpression(member) => {
        Expression::ComputedMemberExpression(member)
      }
      MemberExpression::StaticMemberExpression(member) => {
        Expression::StaticMemberExpression(member)
      }
      MemberExpression::PrivateFieldExpression(member) => {
        Expression::PrivateFieldExpression(member)
      }
    }
  }
}

// ---------------------------------------------------------------------------
// AST walking
// ---------------------------------------------------------------------------

/// Information about a single region (statement block) injection point.
struct RegionInfo;

/// Information about a single branch injection point.
struct BranchInfo;

/// Information about a single function-like node collected from the AST.
struct FunctionInfo {
  /// Display name for the coverage report.
  name: String,
  /// Region injection points within this function.
  regions: Vec<RegionInfo>,
  /// Branch injection points within this function.
  branches: Vec<BranchInfo>,
}

struct FunctionCollector<'src> {
  functions: Vec<FunctionInfo>,
  anon_counter: usize,
  /// Stack of indices into `functions` for nested function scopes.
  func_stack: Vec<usize>,
  _marker: std::marker::PhantomData<&'src str>,
}

impl<'src> FunctionCollector<'src> {
  fn next_anon_name(&mut self) -> String {
    let anon_index = self.anon_counter.to_string();
    let name = CoverageIdentifier::from_parts(["anonymous_", anon_index.as_str()]);
    self.anon_counter += 1;
    name
  }

  fn push_function(&mut self, name: String) -> usize {
    let idx = self.functions.len();
    self.functions.push(FunctionInfo {
      name,
      regions: Vec::new(),
      branches: Vec::new(),
    });
    idx
  }

  /// Return the index of the currently active function, if any.
  fn current_func(&self) -> Option<usize> {
    self.func_stack.last().copied()
  }

  fn add_region(&mut self) {
    if let Some(fi) = self.current_func() {
      self.functions[fi].regions.push(RegionInfo);
    }
  }

  fn add_branch(&mut self) {
    if let Some(fi) = self.current_func() {
      self.functions[fi].branches.push(BranchInfo);
    }
  }

  fn function_name(func: &Function<'_>) -> Option<String> {
    func.id.as_ref().map(|id| id.name.as_str().to_string())
  }

  /// Extract the block span from a `Statement` if it is a `BlockStatement`.
  fn block_span_of_stmt(stmt: &Statement<'_>) -> Option<Span> {
    if let Statement::BlockStatement(block) = stmt {
      Some(block.span)
    } else {
      None
    }
  }
}

impl<'a> Visit<'a> for FunctionCollector<'_> {
  fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
    let name = Self::function_name(it).unwrap_or_else(|| self.next_anon_name());
    let idx = self.push_function(name);
    self.func_stack.push(idx);
    walk::walk_function(self, it, flags);
    self.func_stack.pop();
  }

  fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
    let name = self.next_anon_name();
    let idx = self.push_function(name);
    self.func_stack.push(idx);
    walk::walk_arrow_function_expression(self, it);
    self.func_stack.pop();
  }

  fn visit_if_statement(&mut self, it: &IfStatement<'a>) {
    // Collect branch info before recursing into children.
    let then_block_span = Self::block_span_of_stmt(&it.consequent);
    let else_block_span = it.alternate.as_ref().and_then(Self::block_span_of_stmt);
    if then_block_span.is_some() {
      self.add_branch();
      self.add_region();
    }
    if else_block_span.is_some() {
      self.add_region();
    }

    walk::walk_if_statement(self, it);
  }

  fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
    if Self::block_span_of_stmt(&it.body).is_some() {
      self.add_region();
    }
    walk::walk_while_statement(self, it);
  }

  fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
    if Self::block_span_of_stmt(&it.body).is_some() {
      self.add_region();
    }
    walk::walk_do_while_statement(self, it);
  }

  fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
    if Self::block_span_of_stmt(&it.body).is_some() {
      self.add_region();
    }
    walk::walk_for_statement(self, it);
  }

  fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
    if Self::block_span_of_stmt(&it.body).is_some() {
      self.add_region();
    }
    walk::walk_for_in_statement(self, it);
  }

  fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
    if Self::block_span_of_stmt(&it.body).is_some() {
      self.add_region();
    }
    walk::walk_for_of_statement(self, it);
  }

  fn visit_try_statement(&mut self, it: &TryStatement<'a>) {
    // try block
    self.add_region();
    // catch block
    if it.handler.is_some() {
      self.add_region();
    }
    // finally block
    if it.finalizer.is_some() {
      self.add_region();
    }
    walk::walk_try_statement(self, it);
  }

  fn visit_switch_statement(&mut self, it: &SwitchStatement<'a>) {
    for _case in &it.cases {
      self.add_branch();
      self.add_region();
    }
    walk::walk_switch_statement(self, it);
  }

  fn visit_conditional_expression(&mut self, it: &ConditionalExpression<'a>) {
    self.add_branch();
    walk::walk_conditional_expression(self, it);
  }
}

struct CoverageAstInstrumenter<'a> {
  ast: AstBuilder<'a>,
  call_emitter: CoverageCallEmitter<'a>,
  next_function_index: usize,
  function_stack: Vec<FunctionInstrumentationState>,
}

impl<'a> CoverageAstInstrumenter<'a> {
  fn new(allocator: &'a Allocator, bcv_var: &'a str) -> Self {
    Self {
      ast: AstBuilder::new(allocator),
      call_emitter: CoverageCallEmitter::new(bcv_var),
      next_function_index: 0,
      function_stack: Vec::new(),
    }
  }

  fn push_function(&mut self) -> usize {
    let function_index = self.next_function_index;
    self.next_function_index += 1;
    self
      .function_stack
      .push(FunctionInstrumentationState::new(function_index));
    function_index
  }

  fn pop_function(&mut self) {
    self.function_stack.pop();
  }

  fn next_region_index(&mut self) -> Option<(usize, usize)> {
    self
      .function_stack
      .last_mut()
      .map(FunctionInstrumentationState::next_region)
  }

  fn next_branch_index(&mut self) -> Option<(usize, usize)> {
    self
      .function_stack
      .last_mut()
      .map(FunctionInstrumentationState::next_branch)
  }

  fn prepend_statement(&self, body: &mut OxcVec<'a, Statement<'a>>, statement: Statement<'a>) {
    let old_body = body.take_in(self.ast);
    body.push(statement);
    body.extend(old_body);
  }

  fn prepend_statements<const N: usize>(
    &self,
    body: &mut OxcVec<'a, Statement<'a>>,
    statements: [Statement<'a>; N],
  ) {
    let old_body = body.take_in(self.ast);
    body.extend(self.ast.vec_from_iter(statements));
    body.extend(old_body);
  }

  fn prepend_region_statement(&mut self, body: &mut OxcVec<'a, Statement<'a>>) {
    if let Some((function_index, region_index)) = self.next_region_index() {
      let statement = self
        .call_emitter
        .region_statement(self.ast, function_index, region_index);
      self.prepend_statement(body, statement);
    }
  }

  fn prepend_branch_and_region(
    &mut self,
    body: &mut OxcVec<'a, Statement<'a>>,
    true_or_false: bool,
  ) {
    let Some((function_index, branch_index)) = self.next_branch_index() else {
      return;
    };
    let branch_statement =
      self
        .call_emitter
        .branch_statement(self.ast, function_index, branch_index, true_or_false);
    if let Some((function_index, region_index)) = self.next_region_index() {
      self.prepend_statements(
        body,
        [
          branch_statement,
          self
            .call_emitter
            .region_statement(self.ast, function_index, region_index),
        ],
      );
    } else {
      self.prepend_statement(body, branch_statement);
    }
  }

  fn block_body_mut<'b>(
    statement: &'b mut Statement<'a>,
  ) -> Option<&'b mut OxcVec<'a, Statement<'a>>> {
    if let Statement::BlockStatement(block) = statement {
      Some(&mut block.body)
    } else {
      None
    }
  }
}

impl<'a> VisitMut<'a> for CoverageAstInstrumenter<'a> {
  fn visit_function(&mut self, function: &mut Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
    let function_index = self.push_function();
    if let Some(body) = function.body.as_mut() {
      let statement = self
        .call_emitter
        .function_entry_statement(self.ast, function_index);
      self.prepend_statement(&mut body.statements, statement);
    }
    walk_mut::walk_function(self, function, flags);
    self.pop_function();
  }

  fn visit_arrow_function_expression(&mut self, arrow: &mut ArrowFunctionExpression<'a>) {
    let function_index = self.push_function();
    if !arrow.expression {
      let statement = self
        .call_emitter
        .function_entry_statement(self.ast, function_index);
      self.prepend_statement(&mut arrow.body.statements, statement);
    }
    walk_mut::walk_arrow_function_expression(self, arrow);
    self.pop_function();
  }

  fn visit_if_statement(&mut self, statement: &mut IfStatement<'a>) {
    let true_branch = Self::block_body_mut(&mut statement.consequent).and_then(|body| {
      let branch = self.next_branch_index()?;
      let (function_index, branch_index) = branch;
      let branch_statement =
        self
          .call_emitter
          .branch_statement(self.ast, function_index, branch_index, true);
      if let Some((function_index, region_index)) = self.next_region_index() {
        self.prepend_statements(
          body,
          [
            branch_statement,
            self
              .call_emitter
              .region_statement(self.ast, function_index, region_index),
          ],
        );
      } else {
        self.prepend_statement(body, branch_statement);
      }
      Some(branch)
    });

    match statement.alternate.as_mut().and_then(Self::block_body_mut) {
      Some(body) => {
        if let Some((function_index, branch_index)) = true_branch {
          let branch_statement =
            self
              .call_emitter
              .branch_statement(self.ast, function_index, branch_index, false);
          if let Some((function_index, region_index)) = self.next_region_index() {
            self.prepend_statements(
              body,
              [
                branch_statement,
                self
                  .call_emitter
                  .region_statement(self.ast, function_index, region_index),
              ],
            );
          } else {
            self.prepend_statement(body, branch_statement);
          }
        } else {
          self.prepend_region_statement(body);
        }
      }
      None if true_branch.is_some() => {
        let (function_index, branch_index) = true_branch.expect("checked is_some");
        let branch_statement =
          self
            .call_emitter
            .branch_statement(self.ast, function_index, branch_index, false);
        statement.alternate = if let Some(alternate) = statement.alternate.take() {
          Some(
            self
              .ast
              .statement_block(SPAN, self.ast.vec_from_iter([branch_statement, alternate])),
          )
        } else {
          Some(
            self
              .ast
              .statement_block(SPAN, self.ast.vec1(branch_statement)),
          )
        };
      }
      None => {}
    }

    walk_mut::walk_if_statement(self, statement);
  }

  fn visit_while_statement(&mut self, statement: &mut WhileStatement<'a>) {
    if let Some(body) = Self::block_body_mut(&mut statement.body) {
      self.prepend_region_statement(body);
    }
    walk_mut::walk_while_statement(self, statement);
  }

  fn visit_do_while_statement(&mut self, statement: &mut DoWhileStatement<'a>) {
    if let Some(body) = Self::block_body_mut(&mut statement.body) {
      self.prepend_region_statement(body);
    }
    walk_mut::walk_do_while_statement(self, statement);
  }

  fn visit_for_statement(&mut self, statement: &mut ForStatement<'a>) {
    if let Some(body) = Self::block_body_mut(&mut statement.body) {
      self.prepend_region_statement(body);
    }
    walk_mut::walk_for_statement(self, statement);
  }

  fn visit_for_in_statement(&mut self, statement: &mut ForInStatement<'a>) {
    if let Some(body) = Self::block_body_mut(&mut statement.body) {
      self.prepend_region_statement(body);
    }
    walk_mut::walk_for_in_statement(self, statement);
  }

  fn visit_for_of_statement(&mut self, statement: &mut ForOfStatement<'a>) {
    if let Some(body) = Self::block_body_mut(&mut statement.body) {
      self.prepend_region_statement(body);
    }
    walk_mut::walk_for_of_statement(self, statement);
  }

  fn visit_try_statement(&mut self, statement: &mut TryStatement<'a>) {
    self.prepend_region_statement(&mut statement.block.body);
    if let Some(handler) = statement.handler.as_mut() {
      self.prepend_region_statement(&mut handler.body.body);
    }
    if let Some(finalizer) = statement.finalizer.as_mut() {
      self.prepend_region_statement(&mut finalizer.body);
    }
    walk_mut::walk_try_statement(self, statement);
  }

  fn visit_switch_statement(&mut self, statement: &mut SwitchStatement<'a>) {
    for case in &mut statement.cases {
      self.prepend_branch_and_region(&mut case.consequent, true);
    }
    walk_mut::walk_switch_statement(self, statement);
  }

  fn visit_conditional_expression(&mut self, expression: &mut ConditionalExpression<'a>) {
    let Some((function_index, branch_index)) = self.next_branch_index() else {
      walk_mut::walk_conditional_expression(self, expression);
      return;
    };
    let consequent = expression.consequent.take_in(self.ast);
    expression.consequent = self.ast.expression_parenthesized(
      SPAN,
      self.ast.expression_sequence(
        SPAN,
        self.ast.vec_from_iter([
          self
            .call_emitter
            .branch_call_expression(self.ast, function_index, branch_index, true),
          consequent,
        ]),
      ),
    );
    let alternate = expression.alternate.take_in(self.ast);
    expression.alternate = self.ast.expression_parenthesized(
      SPAN,
      self.ast.expression_sequence(
        SPAN,
        self.ast.vec_from_iter([
          self
            .call_emitter
            .branch_call_expression(self.ast, function_index, branch_index, false),
          alternate,
        ]),
      ),
    );
    walk_mut::walk_conditional_expression(self, expression);
  }
}

struct FunctionInstrumentationState {
  function_index: usize,
  next_region_index: usize,
  next_branch_index: usize,
}

impl FunctionInstrumentationState {
  fn new(function_index: usize) -> Self {
    Self {
      function_index,
      next_region_index: 1,
      next_branch_index: 0,
    }
  }

  fn next_region(&mut self) -> (usize, usize) {
    let index = self.next_region_index;
    self.next_region_index += 1;
    (self.function_index, index)
  }

  fn next_branch(&mut self) -> (usize, usize) {
    let index = self.next_branch_index;
    self.next_branch_index += 1;
    (self.function_index, index)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn injects_bjc_cov_class() {
    let source = "function foo() { return 1; }";
    let result = CoverageInstrumenter::new(source, "test.js").instrument();
    assert!(
      result.contains("class BjcCov"),
      "expected BjcCov class in output"
    );
  }

  #[test]
  fn injects_instrument_function_call() {
    let source = "function foo() { return 1; }";
    let result = CoverageInstrumenter::new(source, "test.js").instrument();
    assert!(
      result.contains("instrumentFunction(0)"),
      "expected instrumentFunction(0) call; got:\n{result}"
    );
  }

  #[test]
  fn handles_multiple_functions() {
    let source = "function foo() { return 1; }\nfunction bar() { return 2; }";
    let result = CoverageInstrumenter::new(source, "multi.js").instrument();
    assert!(
      result.contains("instrumentFunction(0)"),
      "missing call for foo"
    );
    assert!(
      result.contains("instrumentFunction(1)"),
      "missing call for bar"
    );
  }

  #[test]
  fn handles_arrow_function() {
    let source = "const f = () => { return 42; };";
    let result = CoverageInstrumenter::new(source, "arrow.js").instrument();
    assert!(
      result.contains("instrumentFunction(0)"),
      "expected instrumentFunction for arrow; got:\n{result}"
    );
  }

  #[test]
  fn graceful_degradation_on_parse_error() {
    // Severely broken JS — OXC may still parse it partially, but we test
    // that the instrumenter never panics and returns a non-empty string.
    let source = "function (((broken";
    let result = CoverageInstrumenter::new(source, "bad.js").instrument();
    assert!(!result.is_empty(), "result must not be empty");
  }

  #[test]
  fn expression_arrow_has_no_brace_injection() {
    // `() => expr` has no block body — we still record the function but
    // cannot inject a call inside it.  The output must not panic.
    let source = "const f = x => x * 2;";
    let result = CoverageInstrumenter::new(source, "expr_arrow.js").instrument();
    assert!(
      result.contains("class BjcCov"),
      "BjcCov class must be present"
    );
  }

  #[test]
  fn no_string_counting_heuristics() {
    // The old implementation counted `;` and `{` in strings.
    // Verify we no longer do that by checking the function count in covData
    // is driven by actual AST nodes, not character counts.
    let source = r#"const s = "function fake() { ; ; ; }"; function real() { return s; }"#;
    let result = CoverageInstrumenter::new(source, "heuristic.js").instrument();
    // Only one real function (`real`), so only one entry in functions array.
    // The BjcCov class itself contains `instrumentFunction` but only one
    // `instrumentFunction(0)` injection should appear in the source body.
    let count = result.matches("instrumentFunction(0)").count();
    // One call injected into `real()` body, plus the method definition in BjcCov.
    // The method definition does NOT contain `instrumentFunction(0)` — it contains
    // `this.coverage.functions[func].count++`.  So exactly one injection.
    assert_eq!(
      count, 1,
      "expected exactly one instrumentFunction(0) injection"
    );
  }

  #[test]
  fn injects_instrument_region_for_if_blocks() {
    let source = "function foo() { if (x) { doA(); } else { doB(); } }";
    let result = CoverageInstrumenter::new(source, "if.js").instrument();
    assert!(
      result.contains("instrumentRegion(0,"),
      "expected instrumentRegion for if blocks; got:\n{result}"
    );
  }

  #[test]
  fn injects_instrument_branch_for_if() {
    let source = "function foo() { if (x) { doA(); } else { doB(); } }";
    let result = CoverageInstrumenter::new(source, "if_branch.js").instrument();
    assert!(
      result.contains("instrumentBranch(0, 0, true)"),
      "expected true branch; got:\n{result}"
    );
    assert!(
      result.contains("instrumentBranch(0, 0, false)"),
      "expected false branch; got:\n{result}"
    );
  }

  #[test]
  fn injects_else_block_for_if_without_else() {
    let source = "function foo() { if (x) { doA(); } }";
    let result = CoverageInstrumenter::new(source, "no_else.js").instrument();
    assert!(
      result.contains("instrumentBranch(0, 0, false)"),
      "expected synthetic else branch; got:\n{result}"
    );
  }

  #[test]
  fn non_block_if_keeps_collector_and_instrumentation_counts_aligned() {
    let source = "function foo() { if (x) doA(); else { doB(); } }";
    let result = CoverageInstrumenter::new(source, "if_non_block.js").instrument();
    assert!(
      !result.contains("instrumentBranch(0,"),
      "non-block consequent must not emit unregistered branch calls; got:\n{result}"
    );
    assert!(
      result.contains("instrumentRegion(0, 1)"),
      "else block region should still be instrumented; got:\n{result}"
    );
  }

  #[test]
  fn injects_instrument_region_for_while() {
    let source = "function foo() { while (cond) { doA(); } }";
    let result = CoverageInstrumenter::new(source, "while.js").instrument();
    assert!(
      result.contains("instrumentRegion(0,"),
      "expected instrumentRegion for while body; got:\n{result}"
    );
  }

  #[test]
  fn injects_instrument_region_for_try_catch() {
    let source = "function foo() { try { doA(); } catch(e) { doB(); } finally { doC(); } }";
    let result = CoverageInstrumenter::new(source, "try.js").instrument();
    // Should have 3 regions: try block, catch block, finally block.
    let count = result.matches("instrumentRegion(0,").count();
    assert!(
      count >= 3,
      "expected >=3 instrumentRegion calls for try/catch/finally; got {count}; output:\n{result}"
    );
  }

  #[test]
  fn injects_instrument_branch_for_switch() {
    let source = "function foo() { switch(x) { case 1: doA(); break; default: doB(); } }";
    let result = CoverageInstrumenter::new(source, "switch.js").instrument();
    assert!(
      result.contains("instrumentBranch(0,"),
      "expected instrumentBranch for switch cases; got:\n{result}"
    );
  }

  #[test]
  fn injects_comma_expr_for_ternary() {
    let source = "function foo() { return x ? a : b; }";
    let result = CoverageInstrumenter::new(source, "ternary.js").instrument();
    assert!(
      result.contains("instrumentBranch(0, 0, true)"),
      "expected true ternary branch; got:\n{result}"
    );
    assert!(
      result.contains("instrumentBranch(0, 0, false)"),
      "expected false ternary branch; got:\n{result}"
    );
  }

  #[test]
  fn regions_in_json_match_collected_count() {
    // A function with an if/else should have 3 regions in covData:
    // region[0] = function entry, region[1] = then block, region[2] = else block.
    let source = "function foo() { if (x) { doA(); } else { doB(); } }";
    let result = CoverageInstrumenter::new(source, "regions.js").instrument();
    // Count "{ count: 0 }" occurrences in the regions array for foo.
    // The BjcCov class has "count++" not "count: 0", so these are only in covData.
    let region_count = result.matches("{ count: 0 }").count();
    assert!(
      region_count >= 3,
      "expected >=3 region entries; got {region_count}"
    );
  }
}
