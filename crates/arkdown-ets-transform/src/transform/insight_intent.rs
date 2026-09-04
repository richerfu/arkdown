//! ArkTS 1.1 InsightIntent decorator extraction.
//!
//! The SDK parser runs from `process_ui_syntax` and mutates process-global
//! maps before `buildEnd` writes the resource profile. Arkdown keeps the same
//! per-source facts on the transform result instead: the Rolldown adapter can
//! collect them without filesystem I/O or a JavaScript singleton.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result as AnyResult};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use arkdown_core::InsightIntentFormBinding;
use oxc_allocator::Vec as OxcVec;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, BindingPattern, Class, ClassElement, Declaration, Decorator,
    ExportDefaultDeclarationKind, Expression, ImportDeclarationSpecifier, MethodDefinition,
    ObjectPropertyKind, Program, PropertyDefinition, Statement, StructElement, StructStatement,
    TSClassImplements, TSType, TSTypeName, VariableDeclaration, VariableDeclarationKind,
};
use oxc_span::{GetSpan, Span};

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::ImportedInsightIntentEntities;

const PAGE: &str = "InsightIntentPage";
const LINK: &str = "InsightIntentLink";
const ENTRY: &str = "InsightIntentEntry";
const FUNCTION: &str = "InsightIntentFunction";
const METHOD: &str = "InsightIntentFunctionMethod";
const ENTITY: &str = "InsightIntentEntity";
const FORM: &str = "InsightIntentForm";

const OWNER_DECORATORS: [&str; 6] = [LINK, ENTRY, FUNCTION, PAGE, ENTITY, FORM];
const ALL_DECORATORS: [&str; 7] = [LINK, ENTRY, FUNCTION, METHOD, PAGE, ENTITY, FORM];
const INSIGHT_INTENT_DECORATOR_MODULE: &str = "@ohos.app.ability.InsightIntentDecorator";
const ABILITY_KIT_MODULE: &str = "@kit.AbilityKit";
const BASE_REQUIRED: [&str; 4] = ["intentName", "domain", "intentVersion", "displayName"];
const BASE_ALLOWED: [&str; 12] = [
    "intentName",
    "domain",
    "intentVersion",
    "displayName",
    "displayDescription",
    "schema",
    "icon",
    "llmDescription",
    "keywords",
    "parameters",
    "result",
    "example",
];

/// One extracted intent before package ownership turns its module id into a
/// normalized OHM URL.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InsightIntentDeclaration {
    pub data: Map<String, Value>,
    /// Entity class names referenced by entry/form class properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_type_names: Vec<String>,
}

/// Entity declarations are kept separate, matching the SDK's `entityMap`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InsightIntentEntityDeclaration {
    pub class_name: String,
    pub data: Map<String, Value>,
    pub parent_class_name: Option<String>,
}

/// Facts emitted by a single ETS source transform.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InsightIntentSourceMetadata {
    pub intents: Vec<InsightIntentDeclaration>,
    pub entities: Vec<InsightIntentEntityDeclaration>,
    /// Class symbols kept by the SDK's `arkUIKeepInfo.globalNames` inventory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub obfuscation_global_names: Vec<String>,
    /// Class/member symbols kept by `arkUIKeepInfo.propertyNames`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub obfuscation_property_names: Vec<String>,
}

/// SDK-provided built-in intent schema catalog loaded once before Rolldown
/// starts. Transform hooks only perform immutable map lookups.
#[derive(Debug, Clone, Default)]
pub struct InsightIntentSchemaCatalog {
    schemas: Arc<FxHashMap<String, Map<String, Value>>>,
}

impl InsightIntentSchemaCatalog {
    pub fn from_ets_loader_path(ets_loader_path: &Path) -> AnyResult<Self> {
        let schema_root = ets_loader_path.join("insight_intents").join("schema");
        if !schema_root.is_dir() {
            return Ok(Self::default());
        }
        let mut paths = fs_err::read_dir(&schema_root)
            .with_context(|| {
                format!(
                    "read InsightIntent schema catalog {}",
                    schema_root.display()
                )
            })?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some("json")
            })
            .collect::<Vec<_>>();
        paths.sort();
        let mut schemas = FxHashMap::default();
        for path in paths {
            let Some(key) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let source = fs_err::read_to_string(&path)
                .with_context(|| format!("read InsightIntent schema {}", path.display()))?;
            let schema = serde_json::from_str::<Value>(&source)
                .with_context(|| format!("parse InsightIntent schema {}", path.display()))?
                .as_object()
                .cloned()
                .with_context(|| {
                    format!("InsightIntent schema {} must be an object", path.display())
                })?;
            schemas.insert(key.to_string(), schema);
        }
        Ok(Self {
            schemas: Arc::new(schemas),
        })
    }

    fn get(&self, schema: &str, version: &str) -> Option<&Map<String, Value>> {
        self.schemas.get(&format!("{schema}_{version}"))
    }

    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

impl InsightIntentSourceMetadata {
    pub fn is_empty(&self) -> bool {
        self.intents.is_empty() && self.entities.is_empty()
    }

    pub fn has_obfuscation_keep(&self) -> bool {
        !self.obfuscation_global_names.is_empty() || !self.obfuscation_property_names.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsightIntentKind {
    Page,
    Link,
    Entry,
    Function,
    Entity,
    Form,
}

impl InsightIntentKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            PAGE => Some(Self::Page),
            LINK => Some(Self::Link),
            ENTRY => Some(Self::Entry),
            FUNCTION => Some(Self::Function),
            ENTITY => Some(Self::Entity),
            FORM => Some(Self::Form),
            _ => None,
        }
    }

    fn decorator_type(self) -> &'static str {
        match self {
            Self::Page => "@InsightIntentPage",
            Self::Link => "@InsightIntentLink",
            Self::Entry => "@InsightIntentEntry",
            Self::Function => "@InsightIntentFunctionMethod",
            Self::Entity => "@InsightIntentEntity",
            Self::Form => "@InsightIntentForm",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum StaticValue {
    Json(Value),
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaticValueError {
    Circular,
    NonConstant,
    Unsupported,
}

/// Approximates the SDK type checker's declaration-origin guard without doing
/// graph I/O in Rolldown's transform hook. Exact canonical imports from the SDK
/// are accepted, as are ambient SDK globals. A source-local declaration or an
/// import from another module shadows the SDK global and is not extracted.
#[derive(Debug, Default)]
struct InsightIntentDecoratorBindingInventory {
    sdk_bindings: FxHashSet<String>,
    shadowed_bindings: FxHashSet<String>,
}

impl InsightIntentDecoratorBindingInventory {
    fn from_program(program: &Program<'_>) -> Self {
        let mut inventory = Self::default();
        for statement in &program.body {
            match statement {
                Statement::ImportDeclaration(import) => {
                    let sdk_source = matches!(
                        import.source.value.as_str(),
                        INSIGHT_INTENT_DECORATOR_MODULE | ABILITY_KIT_MODULE
                    );
                    let Some(specifiers) = &import.specifiers else {
                        continue;
                    };
                    for specifier in specifiers {
                        let local = match specifier {
                            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                                specifier.local.name.as_str()
                            }
                            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                                specifier.local.name.as_str()
                            }
                            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                                specifier.local.name.as_str()
                            }
                        };
                        inventory.record_binding(local, sdk_source);
                    }
                }
                Statement::VariableDeclaration(declaration) => {
                    inventory.record_variable_declaration(declaration)
                }
                Statement::FunctionDeclaration(function) => {
                    if let Some(identifier) = &function.id {
                        inventory.record_binding(identifier.name.as_str(), false);
                    }
                }
                Statement::ClassDeclaration(class) => {
                    if let Some(identifier) = &class.id {
                        inventory.record_binding(identifier.name.as_str(), false);
                    }
                }
                Statement::ExportDeclaration(export) => {
                    inventory.record_declaration(&export.declaration)
                }
                Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                        if let Some(identifier) = &function.id {
                            inventory.record_binding(identifier.name.as_str(), false);
                        }
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        if let Some(identifier) = &class.id {
                            inventory.record_binding(identifier.name.as_str(), false);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        inventory
    }

    fn record_declaration(&mut self, declaration: &Declaration<'_>) {
        match declaration {
            Declaration::VariableDeclaration(declaration) => {
                self.record_variable_declaration(declaration)
            }
            Declaration::FunctionDeclaration(function) => {
                if let Some(identifier) = &function.id {
                    self.record_binding(identifier.name.as_str(), false);
                }
            }
            Declaration::ClassDeclaration(class) => {
                if let Some(identifier) = &class.id {
                    self.record_binding(identifier.name.as_str(), false);
                }
            }
            Declaration::TSEnumDeclaration(declaration) => {
                self.record_binding(declaration.id.name.as_str(), false);
            }
            Declaration::TSImportEqualsDeclaration(declaration) => {
                self.record_binding(declaration.id.name.as_str(), false);
            }
            _ => {}
        }
    }

    fn record_variable_declaration(&mut self, declaration: &VariableDeclaration<'_>) {
        for declarator in &declaration.declarations {
            for identifier in declarator.id.get_binding_identifiers() {
                self.record_binding(identifier.name.as_str(), false);
            }
        }
    }

    fn record_binding(&mut self, name: &str, sdk_source: bool) {
        if !ALL_DECORATORS.contains(&name) {
            return;
        }
        if sdk_source {
            self.sdk_bindings.insert(name.to_string());
        } else {
            self.shadowed_bindings.insert(name.to_string());
        }
    }

    fn recognizes(&self, name: &str) -> bool {
        ALL_DECORATORS.contains(&name)
            && (self.sdk_bindings.contains(name) || !self.shadowed_bindings.contains(name))
    }
}

struct StaticValueInventory<'p, 'a> {
    source_text: &'p str,
    declarations: FxHashMap<String, &'p Expression<'a>>,
    cache: FxHashMap<String, Result<StaticValue, StaticValueError>>,
    visiting: FxHashSet<String>,
}

impl<'p, 'a> StaticValueInventory<'p, 'a> {
    fn from_program(
        program: &'p Program<'a>,
        source_text: &'p str,
    ) -> FxHashMap<String, Result<StaticValue, StaticValueError>> {
        let mut inventory = Self {
            source_text,
            declarations: FxHashMap::default(),
            cache: FxHashMap::default(),
            visiting: FxHashSet::default(),
        };
        for statement in &program.body {
            match statement {
                Statement::VariableDeclaration(declaration) => inventory.collect(declaration),
                Statement::ExportDeclaration(export) => {
                    if let Declaration::VariableDeclaration(declaration) = &export.declaration {
                        inventory.collect(declaration);
                    }
                }
                _ => {}
            }
        }
        let names = inventory.declarations.keys().cloned().collect::<Vec<_>>();
        for name in names {
            let result = inventory.evaluate_identifier(&name);
            inventory.cache.insert(name, result);
        }
        inventory.cache
    }

    fn collect(&mut self, declaration: &'p VariableDeclaration<'a>) {
        if declaration.kind != VariableDeclarationKind::Const {
            return;
        }
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            if let Some(initializer) = declarator.init.as_ref() {
                self.declarations
                    .insert(identifier.name.to_string(), initializer);
            }
        }
    }

    fn evaluate_identifier(&mut self, name: &str) -> Result<StaticValue, StaticValueError> {
        if let Some(value) = self.cache.get(name) {
            return value.clone();
        }
        if !self.visiting.insert(name.to_string()) {
            return Err(StaticValueError::Circular);
        }
        let result = self
            .declarations
            .get(name)
            .copied()
            .ok_or(StaticValueError::NonConstant)
            .and_then(|expression| self.evaluate(expression));
        self.visiting.remove(name);
        self.cache.insert(name.to_string(), result.clone());
        result
    }

    fn evaluate(
        &mut self,
        expression: &'p Expression<'a>,
    ) -> Result<StaticValue, StaticValueError> {
        match expression {
            Expression::StringLiteral(literal) => {
                Ok(StaticValue::Json(Value::String(literal.value.to_string())))
            }
            Expression::NumericLiteral(literal) => {
                let number = if literal.value.fract() == 0.0
                    && literal.value >= i64::MIN as f64
                    && literal.value <= i64::MAX as f64
                {
                    Some(Number::from(literal.value as i64))
                } else {
                    Number::from_f64(literal.value)
                };
                number
                    .map(Value::Number)
                    .map(StaticValue::Json)
                    .ok_or(StaticValueError::Unsupported)
            }
            Expression::BooleanLiteral(literal) => {
                Ok(StaticValue::Json(Value::Bool(literal.value)))
            }
            Expression::NullLiteral(_) => Ok(StaticValue::Json(Value::Null)),
            Expression::Identifier(identifier) if identifier.name == "undefined" => {
                Ok(StaticValue::Undefined)
            }
            Expression::Identifier(identifier) => {
                self.evaluate_identifier(identifier.name.as_str())
            }
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => template
                .quasis
                .first()
                .and_then(|quasi| quasi.value.cooked.or(Some(quasi.value.raw)))
                .map(|text| StaticValue::Json(Value::String(text.to_string())))
                .ok_or(StaticValueError::Unsupported),
            Expression::ArrayExpression(array) => {
                let mut values = Vec::with_capacity(array.elements.len());
                for element in &array.elements {
                    match element {
                        ArrayExpressionElement::SpreadElement(spread) => {
                            let StaticValue::Json(Value::Array(spread_values)) =
                                self.evaluate(&spread.argument)?
                            else {
                                return Err(StaticValueError::Unsupported);
                            };
                            values.extend(spread_values);
                        }
                        ArrayExpressionElement::Elision(_) => values.push(Value::Null),
                        element => {
                            let Some(expression) = element.as_expression() else {
                                return Err(StaticValueError::Unsupported);
                            };
                            match self.evaluate(expression)? {
                                StaticValue::Json(value) => values.push(value),
                                StaticValue::Undefined => values.push(Value::Null),
                            }
                        }
                    }
                }
                Ok(StaticValue::Json(Value::Array(values)))
            }
            Expression::ObjectExpression(object) => {
                let mut values = Map::new();
                for property in &object.properties {
                    match property {
                        ObjectPropertyKind::ObjectProperty(property) if !property.computed => {
                            let Some(name) = property.key.static_name() else {
                                return Err(StaticValueError::Unsupported);
                            };
                            if let StaticValue::Json(value) = self.evaluate(&property.value)? {
                                values.insert(name.into_owned(), value);
                            }
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            let StaticValue::Json(Value::Object(spread_values)) =
                                self.evaluate(&spread.argument)?
                            else {
                                return Err(StaticValueError::Unsupported);
                            };
                            values.extend(spread_values);
                        }
                        _ => return Err(StaticValueError::Unsupported),
                    }
                }
                Ok(StaticValue::Json(Value::Object(values)))
            }
            Expression::CallExpression(call)
                if matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == "$r")
                    && call.arguments.iter().all(|argument| {
                        matches!(argument.as_expression(), Some(Expression::StringLiteral(_)))
                    }) =>
            {
                let span = call.span();
                let text = self
                    .source_text
                    .get(span.start as usize..span.end as usize)
                    .ok_or(StaticValueError::Unsupported)?;
                Ok(StaticValue::Json(Value::String(text.to_string())))
            }
            Expression::StaticMemberExpression(member) => {
                Self::enum_value(member.property.name.as_str())
            }
            Expression::ComputedMemberExpression(member) => member
                .static_property_name()
                .map(|name| name.to_string())
                .ok_or(StaticValueError::Unsupported)
                .and_then(|name| Self::enum_value(&name)),
            Expression::ParenthesizedExpression(expression) => {
                self.evaluate(&expression.expression)
            }
            Expression::TSAsExpression(expression) => self.evaluate(&expression.expression),
            Expression::TSSatisfiesExpression(expression) => self.evaluate(&expression.expression),
            Expression::TSNonNullExpression(expression) => self.evaluate(&expression.expression),
            _ => Err(StaticValueError::Unsupported),
        }
    }

    fn enum_value(name: &str) -> Result<StaticValue, StaticValueError> {
        let value = match name {
            "UI_ABILITY_FOREGROUND" => "0",
            "UI_ABILITY_BACKGROUND" => "1",
            "UI_EXTENSION_ABILITY" => "2",
            "SERVICE_EXTENSION_ABILITY" => "3",
            "LINK" => "link",
            "WANT" => "want",
            _ => return Err(StaticValueError::Unsupported),
        };
        Ok(StaticValue::Json(Value::String(value.to_string())))
    }
}

pub(super) struct InsightIntentDecoratorProcessor<'a> {
    source_id: &'a str,
    source_text: &'a str,
    bundle_name: &'a str,
    module_name: &'a str,
    form_binding: Option<InsightIntentFormBinding>,
    schema_catalog: InsightIntentSchemaCatalog,
    decorator_bindings: InsightIntentDecoratorBindingInventory,
    entity_class_names: FxHashSet<String>,
    imported_entity_types: ImportedInsightIntentEntities,
    static_values: FxHashMap<String, Result<StaticValue, StaticValueError>>,
    metadata: InsightIntentSourceMetadata,
    changed: bool,
}

pub(super) struct InsightIntentProcessorOptions<'a> {
    pub(super) source_id: &'a str,
    pub(super) source_text: &'a str,
    pub(super) bundle_name: &'a str,
    pub(super) module_name: &'a str,
    pub(super) form_binding: Option<InsightIntentFormBinding>,
    pub(super) schema_catalog: InsightIntentSchemaCatalog,
    pub(super) imported_entity_types: ImportedInsightIntentEntities,
}

pub(super) struct InsightIntentDecoratorProcessorOutput {
    pub(super) changed: bool,
    pub(super) metadata: InsightIntentSourceMetadata,
}

struct InsightIntentOwnerContract<'i, 'a> {
    kind: InsightIntentKind,
    is_struct: bool,
    exported: bool,
    default_exported: bool,
    super_name: Option<&'i str>,
    implements: &'i [TSClassImplements<'a>],
    span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InsightIntentClassProperty {
    name: String,
    schema_type: String,
    is_entity: bool,
}

struct InsightIntentExtractionContext<'i, 'a> {
    class_name: &'i str,
    decorators: &'i [Decorator<'a>],
    super_name: Option<String>,
    super_types: Option<&'i [TSType<'a>]>,
    entity_types: Vec<String>,
    properties: Vec<InsightIntentClassProperty>,
    span: Span,
}

impl<'a> InsightIntentDecoratorProcessor<'a> {
    pub(super) fn new(program: &Program<'a>, options: InsightIntentProcessorOptions<'a>) -> Self {
        let InsightIntentProcessorOptions {
            source_id,
            source_text,
            bundle_name,
            module_name,
            form_binding,
            schema_catalog,
            imported_entity_types,
        } = options;
        let decorator_bindings = InsightIntentDecoratorBindingInventory::from_program(program);
        let entity_class_names = Self::entity_class_names(program, &decorator_bindings);
        Self {
            source_id,
            source_text,
            bundle_name,
            module_name,
            form_binding,
            schema_catalog,
            decorator_bindings,
            entity_class_names,
            imported_entity_types,
            static_values: StaticValueInventory::from_program(program, source_text),
            metadata: InsightIntentSourceMetadata::default(),
            changed: false,
        }
    }

    pub(super) fn program(
        mut self,
        program: &mut Program<'a>,
    ) -> Result<InsightIntentDecoratorProcessorOutput, EtsTransformError> {
        for statement in &mut program.body {
            match statement {
                Statement::ClassDeclaration(class) => self.class(class, false, false)?,
                Statement::StructStatement(structure) => self.structure(structure, false, false)?,
                Statement::ExportDeclaration(export) => match &mut export.declaration {
                    Declaration::ClassDeclaration(class) => self.class(class, true, false)?,
                    Declaration::StructStatement(structure) => {
                        self.structure(structure, true, false)?
                    }
                    _ => {}
                },
                Statement::ExportDefaultDeclaration(export) => match &mut export.declaration {
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        self.class(class, true, true)?
                    }
                    ExportDefaultDeclarationKind::StructStatement(structure) => {
                        self.structure(structure, true, true)?
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(InsightIntentDecoratorProcessorOutput {
            changed: self.changed,
            metadata: self.metadata,
        })
    }

    fn class(
        &mut self,
        class: &mut Class<'a>,
        exported: bool,
        default_exported: bool,
    ) -> Result<(), EtsTransformError> {
        let Some(class_name) = class
            .id
            .as_ref()
            .map(|identifier| identifier.name.to_string())
        else {
            return Ok(());
        };
        let syntactic_owner_kinds = Self::syntactic_owner_kinds(&class.decorators);
        self.validate_detached_methods(
            &class.body.body,
            syntactic_owner_kinds.contains(&InsightIntentKind::Function),
        )?;
        if syntactic_owner_kinds.is_empty() {
            return Ok(());
        }
        self.validate_ets_source(class.span)?;
        let owner_kinds = self.recognized_owner_kinds(&class.decorators);
        let has_recognized_owner = !owner_kinds.is_empty();
        let super_name = class
            .heritage
            .as_ref()
            .and_then(|heritage| Self::expression_name(&heritage.expression));
        let implements = &class.implements;
        let implemented_parent = implements
            .first()
            .and_then(|implementation| Self::type_name(&implementation.expression))
            .and_then(|name| name.rsplit('.').next().map(str::to_string));
        let entity_types = self.class_entity_types(&class.body.body);
        let properties = self.class_properties(&class.body.body);
        for kind in owner_kinds {
            self.validate_owner(InsightIntentOwnerContract {
                kind,
                is_struct: false,
                exported,
                default_exported,
                super_name: super_name.as_deref(),
                implements,
                span: class.span,
            })?;
            self.extract_owner(
                kind,
                &mut class.body.body,
                InsightIntentExtractionContext {
                    class_name: &class_name,
                    decorators: &class.decorators,
                    super_name: if kind == InsightIntentKind::Entity {
                        super_name.clone().or_else(|| implemented_parent.clone())
                    } else {
                        super_name.clone()
                    },
                    super_types: class
                        .heritage
                        .as_ref()
                        .and_then(|heritage| heritage.type_arguments.as_deref())
                        .map(|arguments| arguments.params.as_slice()),
                    entity_types: entity_types.clone(),
                    properties: properties.clone(),
                    span: class.span,
                },
            )?;
        }
        if has_recognized_owner {
            self.record_class_obfuscation(&class_name, exported, &class.body.body);
        }
        self.strip(&mut class.decorators, &mut class.body.body);
        Ok(())
    }

    fn structure(
        &mut self,
        structure: &mut StructStatement<'a>,
        exported: bool,
        default_exported: bool,
    ) -> Result<(), EtsTransformError> {
        let syntactic_owner_kinds = Self::syntactic_owner_kinds(&structure.decorators);
        self.validate_detached_struct_methods(
            &structure.body.body,
            syntactic_owner_kinds.contains(&InsightIntentKind::Function),
        )?;
        if syntactic_owner_kinds.is_empty() {
            return Ok(());
        }
        self.validate_ets_source(structure.span)?;
        let owner_kinds = self.recognized_owner_kinds(&structure.decorators);
        let has_recognized_owner = !owner_kinds.is_empty();
        let super_name = structure
            .super_class
            .as_ref()
            .and_then(Self::expression_name);
        let entity_types = self.struct_entity_types(&structure.body.body);
        let implemented_parent = structure
            .implements
            .first()
            .and_then(|implementation| Self::type_name(&implementation.expression))
            .and_then(|name| name.rsplit('.').next().map(str::to_string));
        let properties = self.struct_properties(&structure.body.body);
        for kind in owner_kinds {
            self.validate_owner(InsightIntentOwnerContract {
                kind,
                is_struct: true,
                exported,
                default_exported,
                super_name: super_name.as_deref(),
                implements: &structure.implements,
                span: structure.span,
            })?;
            self.extract_struct_owner(
                kind,
                &mut structure.body.body,
                InsightIntentExtractionContext {
                    class_name: structure.id.name.as_str(),
                    decorators: &structure.decorators,
                    super_name: if kind == InsightIntentKind::Entity {
                        super_name.clone().or_else(|| implemented_parent.clone())
                    } else {
                        super_name.clone()
                    },
                    super_types: structure
                        .super_type_arguments
                        .as_deref()
                        .map(|arguments| arguments.params.as_slice()),
                    entity_types: entity_types.clone(),
                    properties: properties.clone(),
                    span: structure.span,
                },
            )?;
        }
        if has_recognized_owner {
            self.record_struct_obfuscation(
                structure.id.name.as_str(),
                exported,
                &structure.body.body,
            );
        }
        self.strip_struct(&mut structure.decorators, &mut structure.body.body);
        Ok(())
    }

    fn record_class_obfuscation(
        &mut self,
        class_name: &str,
        exported: bool,
        elements: &[ClassElement<'a>],
    ) {
        Self::record_unique(&mut self.metadata.obfuscation_global_names, class_name);
        if exported {
            Self::record_unique(&mut self.metadata.obfuscation_property_names, class_name);
        }
        for element in elements {
            let name = match element {
                ClassElement::PropertyDefinition(property) => property.key.static_name(),
                ClassElement::MethodDefinition(method) => method.key.static_name(),
                ClassElement::AccessorProperty(property) => property.key.static_name(),
                _ => None,
            };
            if let Some(name) = name {
                Self::record_unique(&mut self.metadata.obfuscation_property_names, name.as_ref());
            }
        }
    }

    fn record_struct_obfuscation(
        &mut self,
        class_name: &str,
        exported: bool,
        elements: &[StructElement<'a>],
    ) {
        Self::record_unique(&mut self.metadata.obfuscation_global_names, class_name);
        if exported {
            Self::record_unique(&mut self.metadata.obfuscation_property_names, class_name);
        }
        for element in elements {
            let name = match element {
                StructElement::PropertyDefinition(property) => property.key.static_name(),
                StructElement::MethodDefinition(method) => method.key.static_name(),
                StructElement::AccessorProperty(property) => property.key.static_name(),
                _ => None,
            };
            if let Some(name) = name {
                Self::record_unique(&mut self.metadata.obfuscation_property_names, name.as_ref());
            }
        }
    }

    fn record_unique(names: &mut Vec<String>, name: &str) {
        if !names.iter().any(|candidate| candidate == name) {
            names.push(name.to_string());
        }
    }

    fn syntactic_owner_kinds(decorators: &[Decorator<'a>]) -> Vec<InsightIntentKind> {
        decorators
            .iter()
            .filter_map(Self::called_decorator_name)
            .filter_map(InsightIntentKind::from_name)
            .collect()
    }

    fn entity_class_names(
        program: &Program<'a>,
        bindings: &InsightIntentDecoratorBindingInventory,
    ) -> FxHashSet<String> {
        let mut names = FxHashSet::default();
        let mut record = |name: Option<&str>, decorators: &[Decorator<'a>]| {
            if bindings.recognizes(ENTITY)
                && decorators
                    .iter()
                    .any(|decorator| Self::called_decorator_name(decorator) == Some(ENTITY))
            {
                if let Some(name) = name {
                    names.insert(name.to_string());
                }
            }
        };
        for statement in &program.body {
            match statement {
                Statement::ClassDeclaration(class) => record(
                    class.id.as_ref().map(|identifier| identifier.name.as_str()),
                    &class.decorators,
                ),
                Statement::StructStatement(structure) => {
                    record(Some(structure.id.name.as_str()), &structure.decorators)
                }
                Statement::ExportDeclaration(export) => match &export.declaration {
                    Declaration::ClassDeclaration(class) => record(
                        class.id.as_ref().map(|identifier| identifier.name.as_str()),
                        &class.decorators,
                    ),
                    Declaration::StructStatement(structure) => {
                        record(Some(structure.id.name.as_str()), &structure.decorators)
                    }
                    _ => {}
                },
                Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => record(
                        class.id.as_ref().map(|identifier| identifier.name.as_str()),
                        &class.decorators,
                    ),
                    ExportDefaultDeclarationKind::StructStatement(structure) => {
                        record(Some(structure.id.name.as_str()), &structure.decorators)
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        names
    }

    fn recognized_owner_kinds(&self, decorators: &[Decorator<'a>]) -> Vec<InsightIntentKind> {
        decorators
            .iter()
            .filter_map(Self::called_decorator_name)
            .filter(|name| self.decorator_bindings.recognizes(name))
            .filter_map(InsightIntentKind::from_name)
            .collect()
    }

    fn called_decorator_name<'d>(decorator: &'d Decorator<'a>) -> Option<&'d str> {
        let Expression::CallExpression(call) = &decorator.expression else {
            return None;
        };
        let Expression::Identifier(identifier) = &call.callee else {
            return None;
        };
        Some(identifier.name.as_str())
    }

    fn called_decorator<'d>(
        &self,
        decorators: &'d [Decorator<'a>],
        name: &str,
    ) -> Option<&'d Decorator<'a>> {
        if name != METHOD && !self.decorator_bindings.recognizes(name) {
            return None;
        }
        decorators
            .iter()
            .find(|decorator| Self::called_decorator_name(decorator) == Some(name))
    }

    fn validate_owner(
        &self,
        contract: InsightIntentOwnerContract<'_, 'a>,
    ) -> Result<(), EtsTransformError> {
        match contract.kind {
            InsightIntentKind::Page if !contract.is_struct => Err(self.error(
                "10110016",
                "@InsightIntentPage must be applied to a struct page.",
                contract.span,
                "Decorate a struct page with @InsightIntentPage",
            )),
            InsightIntentKind::Function if !contract.exported => Err(self.error(
                "10110014",
                "The class decorated with @InsightIntentFunction must be exported.",
                contract.span,
                "Add an export statement",
            )),
            InsightIntentKind::Entry if !contract.default_exported => Err(self.error(
                "10110019",
                "The class decorated with @InsightIntentEntry must be exported as default.",
                contract.span,
                "Use the 'export default' syntax",
            )),
            InsightIntentKind::Entry
                if contract
                    .super_name
                    .is_none_or(|name| name != "InsightIntentEntryExecutor") =>
            {
                Err(self.error(
                    "10110018",
                    "Classes decorated with @InsightIntentEntry must inherit from InsightIntentEntryExecutor.",
                    contract.span,
                    "Add the inheritance",
                ))
            }
            InsightIntentKind::Entity
                if !self.has_intent_entity_heritage(
                    contract.super_name,
                    contract.implements,
                ) =>
            {
                Err(self.error(
                    "10110021",
                    "Classes decorated with @InsightIntentEntity must implement InsightIntent.IntentEntity.",
                    contract.span,
                    "Add the implementation or inherit from a base intent entity",
                ))
            }
            InsightIntentKind::Form
                if !contract.default_exported || self.form_binding.is_none() => Err(self.error(
                "10110022",
                "@InsightIntentForm must be applied to formExtensionAbility.",
                contract.span,
                "Decorate the formExtensionAbility class with @InsightIntentForm",
            )),
            _ => Ok(()),
        }
    }

    fn validate_ets_source(&self, span: Span) -> Result<(), EtsTransformError> {
        let path = self
            .source_id
            .split(['?', '#'])
            .next()
            .unwrap_or(self.source_id);
        if path.ends_with(".ets") {
            return Ok(());
        }
        Err(self.error(
            "10110001",
            "The intent decorator can only be used in .ets files.",
            span,
            "Move it to an .ets file",
        ))
    }

    fn has_intent_entity_heritage(
        &self,
        super_name: Option<&str>,
        implements: &[TSClassImplements<'a>],
    ) -> bool {
        super_name.is_some_and(Self::is_intent_entity_name)
            || implements.iter().any(|implementation| {
                Self::type_name(&implementation.expression)
                    .is_some_and(|name| Self::is_intent_entity_name(&name))
            })
    }

    fn is_intent_entity_name(name: &str) -> bool {
        matches!(
            name.rsplit('.').next(),
            Some("IntentEntity" | "AppIntentEntity")
        )
    }

    fn validate_detached_methods(
        &self,
        elements: &[ClassElement<'a>],
        owner_has_function: bool,
    ) -> Result<(), EtsTransformError> {
        for element in elements {
            if let ClassElement::MethodDefinition(method) = element {
                self.validate_method(method, owner_has_function)?;
            }
        }
        Ok(())
    }

    fn validate_detached_struct_methods(
        &self,
        elements: &[StructElement<'a>],
        owner_has_function: bool,
    ) -> Result<(), EtsTransformError> {
        for element in elements {
            if let StructElement::MethodDefinition(method) = element {
                self.validate_method(method, owner_has_function)?;
            }
        }
        Ok(())
    }

    fn validate_method(
        &self,
        method: &MethodDefinition<'a>,
        owner_has_function: bool,
    ) -> Result<(), EtsTransformError> {
        if self.called_decorator(&method.decorators, METHOD).is_none() {
            return Ok(());
        }
        if !owner_has_function && method.r#static {
            return Err(self.error(
                "10110013",
                "Methods decorated with @InsightIntentFunctionMethod must be in a class decorated with @InsightIntentFunction.",
                method.span,
                "Either move the method or add @InsightIntentFunction to the class",
            ));
        }
        if owner_has_function && !method.r#static {
            return Err(self.error(
                "10110015",
                "Methods decorated with @InsightIntentFunctionMethod must be static.",
                method.span,
                "Change the method to static",
            ));
        }
        Ok(())
    }

    fn extract_owner<'i>(
        &mut self,
        kind: InsightIntentKind,
        elements: &mut OxcVec<'a, ClassElement<'a>>,
        context: InsightIntentExtractionContext<'i, 'a>,
    ) -> Result<(), EtsTransformError> {
        if kind == InsightIntentKind::Function {
            for element in elements.iter() {
                if let ClassElement::MethodDefinition(method) = element {
                    self.extract_function_method(context.class_name, method)?;
                }
            }
            return Ok(());
        }
        let entity_id = self.class_entity_id(elements);
        self.extract_decorator(kind, context, entity_id)
    }

    fn extract_struct_owner<'i>(
        &mut self,
        kind: InsightIntentKind,
        elements: &mut OxcVec<'a, StructElement<'a>>,
        context: InsightIntentExtractionContext<'i, 'a>,
    ) -> Result<(), EtsTransformError> {
        if kind == InsightIntentKind::Function {
            for element in elements.iter() {
                if let StructElement::MethodDefinition(method) = element {
                    self.extract_function_method(context.class_name, method)?;
                }
            }
            return Ok(());
        }
        let entity_id = self.struct_entity_id(elements);
        self.extract_decorator(kind, context, entity_id)
    }

    fn extract_decorator(
        &mut self,
        kind: InsightIntentKind,
        context: InsightIntentExtractionContext<'_, 'a>,
        entity_id: Option<String>,
    ) -> Result<(), EtsTransformError> {
        let InsightIntentExtractionContext {
            class_name,
            decorators,
            super_name,
            super_types,
            entity_types,
            properties,
            span,
        } = context;
        let name = match kind {
            InsightIntentKind::Page => PAGE,
            InsightIntentKind::Link => LINK,
            InsightIntentKind::Entry => ENTRY,
            InsightIntentKind::Entity => ENTITY,
            InsightIntentKind::Form => FORM,
            InsightIntentKind::Function => return Ok(()),
        };
        let Some(decorator) = self.called_decorator(decorators, name) else {
            return Ok(());
        };
        let Some(mut arguments) = self.decorator_arguments(decorator, kind, span)? else {
            return Ok(());
        };
        self.collect_schema_info(&mut arguments);
        if kind == InsightIntentKind::Form {
            let binding = self
                .form_binding
                .as_ref()
                .expect("form owner validation requires a form binding");
            let form_name = arguments
                .get("formName")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !binding.form_names.iter().any(|name| name == form_name) {
                return Err(self.error(
                    "10110023",
                    "formName in @InsightIntentForm must match the widget name registered in formExtensionAbility.",
                    span,
                    "Update formName to match the registered widget name",
                ));
            }
            arguments.insert(
                "abilityName".to_string(),
                Value::String(binding.ability_name.clone()),
            );
        }
        if kind == InsightIntentKind::Entity
            && super_name.as_deref() == Some("AppIntentEntity")
            && entity_id.is_none()
        {
            return Err(self.error(
                "10110008",
                &format!("The entityId is not defined in class {class_name}."),
                span,
                "Add the required parameters as specified in the error message",
            ));
        }
        if let Some(schema) = arguments.get("parameters") {
            self.validate_class_schema(kind, &properties, schema, span)?;
        }
        if kind == InsightIntentKind::Entity {
            self.validate_supported_query(&properties, &arguments, span)?;
        }
        if kind == InsightIntentKind::Entry {
            Self::map_execute_modes(&mut arguments);
            if let Some(parent) = super_name.as_deref() {
                arguments.insert(
                    "ClassInheritanceInfo".to_string(),
                    self.class_inheritance(parent, super_types),
                );
            }
        }
        let mut data = Map::new();
        data.insert(
            if kind == InsightIntentKind::Entity {
                "className"
            } else {
                "decoratorClass"
            }
            .to_string(),
            Value::String(class_name.to_string()),
        );
        if kind != InsightIntentKind::Entity {
            data.insert(
                "bundleName".to_string(),
                Value::String(self.bundle_name.to_string()),
            );
            data.insert(
                "moduleName".to_string(),
                Value::String(self.module_name.to_string()),
            );
        }
        data.insert(
            "decoratorType".to_string(),
            Value::String(kind.decorator_type().to_string()),
        );
        data.extend(arguments);
        if kind == InsightIntentKind::Entity {
            if self
                .metadata
                .entities
                .iter()
                .any(|entity| entity.class_name == class_name)
            {
                return Err(self.error(
                    "10110020",
                    "Multiple @InsightIntentEntity decorators applied to the same class.",
                    span,
                    "Remove duplicates",
                ));
            }
            if let Some(entity_id) = entity_id {
                data.insert("entityId".to_string(), Value::String(entity_id));
            }
            if let Some(parent) = super_name.clone() {
                data.insert("parentClassName".to_string(), Value::String(parent.clone()));
            }
            self.metadata.entities.push(InsightIntentEntityDeclaration {
                class_name: class_name.to_string(),
                data,
                parent_class_name: super_name,
            });
        } else {
            self.metadata.intents.push(InsightIntentDeclaration {
                data,
                entity_type_names: entity_types,
            });
        }
        Ok(())
    }

    fn extract_function_method(
        &mut self,
        class_name: &str,
        method: &MethodDefinition<'a>,
    ) -> Result<(), EtsTransformError> {
        let Some(decorator) = self.called_decorator(&method.decorators, METHOD) else {
            return Ok(());
        };
        let Some(arguments) =
            self.decorator_arguments(decorator, InsightIntentKind::Function, method.span)?
        else {
            return Ok(());
        };
        let mut data = Map::new();
        data.insert(
            "decoratorClass".to_string(),
            Value::String(class_name.to_string()),
        );
        data.insert(
            "bundleName".to_string(),
            Value::String(self.bundle_name.to_string()),
        );
        data.insert(
            "moduleName".to_string(),
            Value::String(self.module_name.to_string()),
        );
        data.insert(
            "decoratorType".to_string(),
            Value::String(InsightIntentKind::Function.decorator_type().to_string()),
        );
        data.insert(
            "functionName".to_string(),
            Value::String(
                method
                    .key
                    .static_name()
                    .map(|name| name.into_owned())
                    .unwrap_or_default(),
            ),
        );
        data.insert(
            "functionParamList".to_string(),
            Value::Array(
                method
                    .value
                    .params
                    .items
                    .iter()
                    .filter_map(|parameter| match &parameter.pattern {
                        BindingPattern::BindingIdentifier(identifier) => {
                            Some(Value::String(identifier.name.to_string()))
                        }
                        _ => None,
                    })
                    .collect(),
            ),
        );
        data.extend(arguments);
        self.metadata.intents.push(InsightIntentDeclaration {
            data,
            entity_type_names: Vec::new(),
        });
        Ok(())
    }

    fn decorator_arguments(
        &self,
        decorator: &Decorator<'a>,
        kind: InsightIntentKind,
        span: Span,
    ) -> Result<Option<Map<String, Value>>, EtsTransformError> {
        let Expression::CallExpression(call) = &decorator.expression else {
            return Ok(None);
        };
        let first_argument = call.arguments.first().and_then(Argument::as_expression);
        if matches!(first_argument, Some(Expression::NullLiteral(_)))
            || matches!(first_argument, Some(Expression::Identifier(identifier)) if identifier.name == "undefined")
        {
            return Ok(None);
        }
        let mut values = Map::new();
        for argument in &call.arguments {
            let Some(expression) = argument.as_expression() else {
                return Err(self.error(
                    "10110005",
                    "Unsupported parameters found in the decorator.",
                    span,
                    "Remove any parameters that are not supported",
                ));
            };
            let result = match expression {
                Expression::Identifier(identifier) => self
                    .static_values
                    .get(identifier.name.as_str())
                    .cloned()
                    .unwrap_or(Err(StaticValueError::NonConstant)),
                _ => {
                    let mut evaluator = StaticValueInventory {
                        source_text: self.source_text,
                        declarations: FxHashMap::default(),
                        cache: self.static_values.clone(),
                        visiting: FxHashSet::default(),
                    };
                    evaluator.evaluate(expression)
                }
            };
            let StaticValue::Json(Value::Object(object)) = result.map_err(|error| {
                let (code, message, solution) = match error {
                    StaticValueError::Circular => (
                        "10110006",
                        "Circular dependencies detected in decorator parameters.",
                        "Refactor the data structure by extracting common variables or using ID references to avoid nesting",
                    ),
                    StaticValueError::NonConstant => (
                        "10110000",
                        "Decorator parameters must be compile-time constants.",
                        "Use a fixed value (such as a string literal) instead of a variable",
                    ),
                    StaticValueError::Unsupported => (
                        "10110005",
                        "Unsupported parameters found in the decorator.",
                        "Remove any parameters that are not supported",
                    ),
                };
                self.error(code, message, span, solution)
            })?
            else {
                return Err(self.error(
                    "10110004",
                    "The parameter type does not match the decorator's requirement.",
                    span,
                    "Adjust the type to match the expected type.",
                ));
            };
            values.extend(object);
        }
        self.validate_arguments(kind, &values, span)?;
        Ok(Some(values))
    }

    fn validate_arguments(
        &self,
        kind: InsightIntentKind,
        values: &Map<String, Value>,
        span: Span,
    ) -> Result<(), EtsTransformError> {
        let mut required = BASE_REQUIRED.to_vec();
        let mut allowed = BASE_ALLOWED.to_vec();
        match kind {
            InsightIntentKind::Link => {
                required.push("uri");
                allowed.extend(["uri", "paramMappings"]);
            }
            InsightIntentKind::Entry => {
                required.extend(["abilityName", "executeMode"]);
                allowed.extend(["abilityName", "executeMode"]);
            }
            InsightIntentKind::Page => {
                required.push("pagePath");
                allowed.extend([
                    "uiAbility",
                    "pagePath",
                    "navigationId",
                    "navDestinationName",
                ]);
            }
            InsightIntentKind::Entity => {
                required = vec!["entityCategory"];
                allowed = vec!["entityCategory", "parameters", "supportedQueryProperties"];
            }
            InsightIntentKind::Form => {
                required.push("formName");
                allowed.push("formName");
            }
            InsightIntentKind::Function => {}
        }
        if required.iter().any(|field| !values.contains_key(*field)) {
            return Err(self.error(
                "10110003",
                "Required parameters are missing for the decorator.",
                span,
                "Add the required parameters as specified in the error message",
            ));
        }
        if values
            .keys()
            .any(|field| !allowed.contains(&field.as_str()))
        {
            return Err(self.error(
                "10110005",
                "Unsupported parameters found in the decorator.",
                span,
                "Remove any parameters that are not supported.",
            ));
        }
        for field in required {
            let requires_non_empty_string = BASE_REQUIRED.contains(&field)
                || matches!(field, "uri" | "abilityName" | "pagePath");
            if !values.get(field).is_some_and(|value| {
                !value.is_null()
                    && (!requires_non_empty_string
                        || matches!(value, Value::String(text) if !text.trim().is_empty()))
            }) {
                return Err(self.error(
                    "10110004",
                    "The parameter type does not match the decorator's requirement.",
                    span,
                    "Adjust the type to match the expected type.",
                ));
            }
        }
        if (kind == InsightIntentKind::Entry
            && values.get("displayDescription").is_some_and(
                |value| !matches!(value, Value::String(text) if !text.trim().is_empty()),
            ))
            || (kind == InsightIntentKind::Form
                && values.get("schema").is_some_and(
                    |value| !matches!(value, Value::String(text) if !text.trim().is_empty()),
                ))
        {
            return Err(self.error(
                "10110004",
                "The parameter type does not match the decorator's requirement.",
                span,
                "Adjust the type to match the expected type.",
            ));
        }
        for field in [
            "intentName",
            "domain",
            "intentVersion",
            "displayName",
            "displayDescription",
            "schema",
            "llmDescription",
            "example",
            "uri",
            "abilityName",
            "pagePath",
            "uiAbility",
            "navigationId",
            "navDestinationName",
            "entityCategory",
            "formName",
        ] {
            if values.get(field).is_some_and(|value| !value.is_string()) {
                return Err(self.error(
                    "10110004",
                    "The parameter type does not match the decorator's requirement.",
                    span,
                    "Adjust the type to match the expected type.",
                ));
            }
        }
        if values
            .get("keywords")
            .is_some_and(|value| !Self::string_array(value))
            || values
                .get("supportedQueryProperties")
                .is_some_and(|value| !Self::string_array(value))
            || values.get("executeMode").is_some_and(|value| {
                !value.as_array().is_some_and(|items| {
                    items.iter().all(|item| {
                        matches!(item, Value::Number(number) if number.as_u64().is_some_and(|number| number <= 3))
                            || matches!(item, Value::String(mode) if matches!(mode.as_str(), "0" | "1" | "2" | "3"))
                    })
                })
            })
            || ["parameters", "result"]
                .iter()
                .any(|field| values.get(*field).is_some_and(|value| !value.is_object()))
        {
            return Err(self.error(
                "10110004",
                "The parameter type does not match the decorator's requirement.",
                span,
                "Adjust the type to match the expected type.",
            ));
        }
        if let Some(Value::Array(mappings)) = values.get("paramMappings") {
            for mapping in mappings {
                let Some(mapping) = mapping.as_object() else {
                    return Err(self.error(
                        "10110004",
                        "The parameter type does not match the decorator's requirement.",
                        span,
                        "Adjust the type to match the expected type.",
                    ));
                };
                if mapping
                    .get("paramName")
                    .and_then(Value::as_str)
                    .is_none_or(|name| name.trim().is_empty())
                {
                    return Err(self.error(
                        "10110003",
                        "Required parameters are missing for the decorator.",
                        span,
                        "Add the required parameters as specified in the error message",
                    ));
                }
                if mapping.keys().any(|field| {
                    !matches!(
                        field.as_str(),
                        "paramName" | "paramMappingName" | "paramCategory"
                    )
                }) {
                    return Err(self.error(
                        "10110005",
                        "Unsupported parameters found in the decorator.",
                        span,
                        "Remove any parameters that are not supported.",
                    ));
                }
                if mapping
                    .get("paramMappingName")
                    .is_some_and(|value| !value.is_string())
                    || mapping
                        .get("paramCategory")
                        .is_some_and(|value| !matches!(value.as_str(), Some("link" | "want")))
                {
                    return Err(self.error(
                        "10110004",
                        "The parameter type does not match the decorator's requirement.",
                        span,
                        "Adjust the type to match the expected type.",
                    ));
                }
            }
        } else if values.contains_key("paramMappings") {
            return Err(self.error(
                "10110004",
                "The parameter type does not match the decorator's requirement.",
                span,
                "Adjust the type to match the expected type.",
            ));
        }
        Ok(())
    }

    fn string_array(value: &Value) -> bool {
        value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string))
    }

    fn map_execute_modes(values: &mut Map<String, Value>) {
        let Some(Value::Array(modes)) = values.get_mut("executeMode") else {
            return;
        };
        for mode in modes {
            let mapped = match mode {
                Value::Number(number) => number.as_u64().and_then(Self::execute_mode),
                Value::String(value) => value.parse::<u64>().ok().and_then(Self::execute_mode),
                _ => None,
            };
            if let Some(mapped) = mapped {
                *mode = Value::String(mapped.to_string());
            }
        }
    }

    fn execute_mode(value: u64) -> Option<&'static str> {
        match value {
            0 => Some("foreground"),
            1 => Some("background"),
            2 => Some("uiextension"),
            3 => Some("serviceextension"),
            _ => None,
        }
    }

    fn class_inheritance(&self, parent: &str, generics: Option<&[TSType<'a>]>) -> Value {
        let generics = generics
            .unwrap_or_default()
            .iter()
            .map(|generic| {
                let type_name = self
                    .source_text
                    .get(generic.span().start as usize..generic.span().end as usize)
                    .unwrap_or_default()
                    .to_string();
                serde_json::json!({
                    "typeName": type_name,
                    "definitionFilePath": if Self::primitive_type(generic) { "lib.es5.d.ts" } else { "" }
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "parentClassName": parent,
            "definitionFilePath": "sdk",
            "generics": generics
        })
    }

    fn primitive_type(r#type: &TSType<'a>) -> bool {
        matches!(
            r#type,
            TSType::TSStringKeyword(_) | TSType::TSNumberKeyword(_) | TSType::TSBooleanKeyword(_)
        )
    }

    fn strip(
        &mut self,
        decorators: &mut OxcVec<'a, Decorator<'a>>,
        elements: &mut OxcVec<'a, ClassElement<'a>>,
    ) {
        self.strip_owner(decorators);
        for element in elements {
            if let ClassElement::MethodDefinition(method) = element {
                self.strip_static_method(method);
            }
        }
    }

    fn strip_struct(
        &mut self,
        decorators: &mut OxcVec<'a, Decorator<'a>>,
        elements: &mut OxcVec<'a, StructElement<'a>>,
    ) {
        self.strip_owner(decorators);
        for element in elements {
            if let StructElement::MethodDefinition(method) = element {
                self.strip_static_method(method);
            }
        }
    }

    fn strip_owner(&mut self, decorators: &mut OxcVec<'a, Decorator<'a>>) {
        let old_len = decorators.len();
        decorators.retain(|decorator| {
            Self::called_decorator_name(decorator)
                .is_none_or(|name| !OWNER_DECORATORS.contains(&name))
        });
        self.changed |= decorators.len() != old_len;
    }

    fn strip_static_method(&mut self, method: &mut MethodDefinition<'a>) {
        if !method.r#static {
            return;
        }
        let old_len = method.decorators.len();
        method
            .decorators
            .retain(|decorator| Self::called_decorator_name(decorator) != Some(METHOD));
        self.changed |= method.decorators.len() != old_len;
    }

    fn expression_name(expression: &Expression<'a>) -> Option<String> {
        match expression {
            Expression::Identifier(identifier) => Some(identifier.name.to_string()),
            Expression::StaticMemberExpression(member) => {
                let object = Self::expression_name(&member.object)?;
                Some(format!("{object}.{}", member.property.name))
            }
            _ => None,
        }
    }

    fn type_name(name: &TSTypeName<'a>) -> Option<String> {
        match name {
            TSTypeName::IdentifierReference(identifier) => Some(identifier.name.to_string()),
            TSTypeName::QualifiedName(qualified) => {
                let left = Self::type_name(&qualified.left)?;
                Some(format!("{left}.{}", qualified.right.name))
            }
            TSTypeName::ThisExpression(_) => Some("this".to_string()),
        }
    }

    fn class_entity_types(&self, elements: &[ClassElement<'a>]) -> Vec<String> {
        elements
            .iter()
            .filter_map(|element| match element {
                ClassElement::PropertyDefinition(property) => {
                    Self::property_entity_type_name(property)
                }
                _ => None,
            })
            .filter_map(|name| self.entity_class_name(&name))
            .collect()
    }

    fn struct_entity_types(&self, elements: &[StructElement<'a>]) -> Vec<String> {
        elements
            .iter()
            .filter_map(|element| match element {
                StructElement::PropertyDefinition(property) => {
                    Self::property_entity_type_name(property)
                }
                _ => None,
            })
            .filter_map(|name| self.entity_class_name(&name))
            .collect()
    }

    fn entity_class_name(&self, name: &str) -> Option<String> {
        if self.entity_class_names.contains(name) {
            return Some(name.to_string());
        }
        if let Some(local_name) = name.rsplit('.').next() {
            if self.entity_class_names.contains(local_name) {
                return Some(local_name.to_string());
            }
        }
        self.imported_entity_types
            .class_name(name)
            .map(str::to_string)
    }

    fn property_entity_type_name(property: &PropertyDefinition<'a>) -> Option<String> {
        let annotation = property.type_annotation.as_deref()?;
        Self::schema_type(&annotation.type_annotation).1
    }

    fn class_properties(&self, elements: &[ClassElement<'a>]) -> Vec<InsightIntentClassProperty> {
        elements
            .iter()
            .filter_map(|element| match element {
                ClassElement::PropertyDefinition(property) => self.class_property(property),
                _ => None,
            })
            .collect()
    }

    fn struct_properties(&self, elements: &[StructElement<'a>]) -> Vec<InsightIntentClassProperty> {
        elements
            .iter()
            .filter_map(|element| match element {
                StructElement::PropertyDefinition(property) => self.class_property(property),
                _ => None,
            })
            .collect()
    }

    fn class_property(
        &self,
        property: &PropertyDefinition<'a>,
    ) -> Option<InsightIntentClassProperty> {
        let name = property.key.static_name()?.into_owned();
        let (schema_type, referenced_name) = property
            .type_annotation
            .as_deref()
            .map(|annotation| Self::schema_type(&annotation.type_annotation))
            .unwrap_or_else(|| (String::new(), None));
        let is_entity = referenced_name
            .as_deref()
            .and_then(|name| self.entity_class_name(name))
            .is_some();
        Some(InsightIntentClassProperty {
            name,
            schema_type,
            is_entity,
        })
    }

    fn schema_type(r#type: &TSType<'a>) -> (String, Option<String>) {
        match r#type {
            TSType::TSStringKeyword(_) => ("string".to_string(), None),
            TSType::TSNumberKeyword(_) => ("number".to_string(), None),
            TSType::TSBooleanKeyword(_) => ("boolean".to_string(), None),
            TSType::TSNullKeyword(_) => ("null".to_string(), None),
            TSType::TSArrayType(array) => {
                let (_, entity_name) = Self::schema_type(&array.element_type);
                ("array".to_string(), entity_name)
            }
            TSType::TSTupleType(_) => ("array".to_string(), None),
            TSType::TSTypeLiteral(_) | TSType::TSObjectKeyword(_) => ("object".to_string(), None),
            TSType::TSTypeReference(reference) => {
                let name = Self::type_name(&reference.type_name);
                if name.as_deref().and_then(|name| name.rsplit('.').next()) == Some("Array") {
                    let entity_name = reference
                        .type_arguments
                        .as_deref()
                        .and_then(|arguments| arguments.params.first())
                        .and_then(|argument| Self::schema_type(argument).1);
                    ("array".to_string(), entity_name)
                } else {
                    ("object".to_string(), name)
                }
            }
            TSType::TSUnionType(union) => {
                let mut members = union.types.iter().filter(|member| {
                    !matches!(
                        member,
                        TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_)
                    )
                });
                let Some(first) = members.next() else {
                    return (String::new(), None);
                };
                if members.next().is_none() {
                    Self::schema_type(first)
                } else {
                    (String::new(), None)
                }
            }
            TSType::TSParenthesizedType(parenthesized) => {
                Self::schema_type(&parenthesized.type_annotation)
            }
            _ => (String::new(), None),
        }
    }

    fn validate_class_schema(
        &self,
        kind: InsightIntentKind,
        properties: &[InsightIntentClassProperty],
        schema: &Value,
        span: Span,
    ) -> Result<(), EtsTransformError> {
        let Some(schema) = schema.as_object() else {
            return Ok(());
        };
        if schema.get("type").and_then(Value::as_str) != Some("object") {
            return Err(self.error(
                "10110007",
                "The root type of the JSON Schema for Parameters must be object.",
                span,
                "Change the top-level definition to {\"type\":\"object\", ...}",
            ));
        }
        let schema_properties = schema.get("properties").and_then(Value::as_object);
        if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
            for property in properties {
                if Self::schema_property_blacklisted(kind, &property.name) {
                    continue;
                }
                if schema_properties.is_none_or(|schema| !schema.contains_key(&property.name)) {
                    return Err(self.error(
                        "10110011",
                        "The class property includes parameters not defined in the JSON Schema.",
                        span,
                        "Remove any extra parameters",
                    ));
                }
            }
        }
        if let Some(schema_properties) = schema_properties {
            for property in properties {
                if Self::schema_property_blacklisted(kind, &property.name) {
                    continue;
                }
                let Some(expected) = schema_properties
                    .get(&property.name)
                    .and_then(Value::as_object)
                else {
                    continue;
                };
                let expected_type = expected.get("type").and_then(Value::as_str);
                if (!property.schema_type.is_empty()
                    && expected_type.is_some_and(|expected| expected != property.schema_type))
                    || (expected_type == Some("object") && !property.is_entity)
                {
                    return Err(self.error(
                        "10110009",
                        "The field type of the class property does not match the JSON Schema.",
                        span,
                        "Correct the type to match the requirement",
                    ));
                }
            }
        }
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for required in required.iter().filter_map(Value::as_str) {
                if !properties.iter().any(|property| property.name == required) {
                    return Err(self.error(
                        "10110008",
                        "A required field in the class property is missing.",
                        span,
                        "Add the required field as specified by the JSON Schema",
                    ));
                }
            }
        }
        self.validate_schema_alternatives(schema, properties, span)
    }

    fn collect_schema_info(&self, arguments: &mut Map<String, Value>) {
        let Some(schema_name) = arguments.get("schema").and_then(Value::as_str) else {
            return;
        };
        let Some(version) = arguments.get("intentVersion").and_then(Value::as_str) else {
            return;
        };
        let Some(schema) = self.schema_catalog.get(schema_name, version).cloned() else {
            return;
        };
        for field in [
            "parameters",
            "llmDescription",
            "keywords",
            "intentName",
            "result",
            "domain",
        ] {
            if let Some(value) = schema.get(field) {
                arguments.insert(field.to_string(), value.clone());
            } else {
                arguments.remove(field);
            }
        }
    }

    fn schema_property_blacklisted(kind: InsightIntentKind, name: &str) -> bool {
        match kind {
            InsightIntentKind::Entry => matches!(
                name,
                "executeMode" | "context" | "windowStage" | "uiExtensionSession" | "onExecute"
            ),
            InsightIntentKind::Form => name == "context",
            _ => false,
        }
    }

    fn validate_schema_alternatives(
        &self,
        schema: &Map<String, Value>,
        properties: &[InsightIntentClassProperty],
        span: Span,
    ) -> Result<(), EtsTransformError> {
        let matches_required = |candidate: &Value| {
            candidate
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|required| {
                    required
                        .iter()
                        .filter_map(Value::as_str)
                        .all(|name| properties.iter().any(|property| property.name == name))
                })
        };
        let one_of_matches = schema
            .get("oneOf")
            .and_then(Value::as_array)
            .map(|candidates| {
                candidates
                    .iter()
                    .filter(|candidate| matches_required(candidate))
                    .count()
            });
        let any_of_matches = schema
            .get("anyOf")
            .and_then(Value::as_array)
            .map(|candidates| {
                candidates
                    .iter()
                    .filter(|candidate| matches_required(candidate))
                    .count()
            });
        if one_of_matches.is_some_and(|count| count != 1)
            || any_of_matches.is_some_and(|count| count == 0)
        {
            return Err(self.error(
                "10110010",
                "The class property parameter violates the oneOf/anyOf validation rules in the JSON Schema.",
                span,
                "Modify it to satisfy the rules",
            ));
        }
        Ok(())
    }

    fn validate_supported_query(
        &self,
        properties: &[InsightIntentClassProperty],
        arguments: &Map<String, Value>,
        span: Span,
    ) -> Result<(), EtsTransformError> {
        let Some(supported) = arguments
            .get("supportedQueryProperties")
            .and_then(Value::as_array)
            .filter(|supported| !supported.is_empty())
        else {
            return Ok(());
        };
        let schema_properties = arguments
            .get("parameters")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object);
        for field in supported.iter().filter_map(Value::as_str) {
            if schema_properties.is_none_or(|schema| !schema.contains_key(field)) {
                return Err(self.error(
                    "10110003",
                    &format!(
                        "The supportedQueryProperties field '{field}' is not defined in properties."
                    ),
                    span,
                    "Add the required parameters as specified in the error message",
                ));
            }
            if !properties.iter().any(|property| property.name == field) {
                return Err(self.error(
                    "10110003",
                    &format!(
                        "The field '{field}' in supportedQueryProperties is not a property of the class."
                    ),
                    span,
                    "Add the required parameters as specified in the error message",
                ));
            }
        }
        Ok(())
    }

    fn class_entity_id(&self, elements: &[ClassElement<'a>]) -> Option<String> {
        elements.iter().find_map(|element| match element {
            ClassElement::PropertyDefinition(property)
                if property.key.static_name().as_deref() == Some("entityId") =>
            {
                self.string_initializer(property)
            }
            _ => None,
        })
    }

    fn struct_entity_id(&self, elements: &[StructElement<'a>]) -> Option<String> {
        elements.iter().find_map(|element| match element {
            StructElement::PropertyDefinition(property)
                if property.key.static_name().as_deref() == Some("entityId") =>
            {
                self.string_initializer(property)
            }
            _ => None,
        })
    }

    fn string_initializer(&self, property: &PropertyDefinition<'a>) -> Option<String> {
        let expression = property.value.as_ref()?;
        match expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            Expression::Identifier(identifier) => self
                .static_values
                .get(identifier.name.as_str())
                .and_then(|value| value.as_ref().ok())
                .and_then(|value| match value {
                    StaticValue::Json(Value::String(value)) => Some(value.clone()),
                    _ => None,
                }),
            _ => None,
        }
    }

    fn error(&self, code: &str, message: &str, span: Span, solution: &str) -> EtsTransformError {
        EtsTransformError::CodedDiagnostic {
            file: self.source_id.to_string(),
            code: code.to_string(),
            message: message.to_string(),
            severity: DiagnosticSeverity::Error,
            span: Some(span),
            solutions: vec![solution.to_string()],
        }
    }
}
