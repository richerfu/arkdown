//! OXC-based static parser for `hvigorfile.ts` / `hvigorconfig.ts`.
//!
//! `BuildScriptParser` reads a build script file, parses it with OXC, and
//! extracts the declarative [`BuildScriptEffect`]s that can be represented
//! without executing JavaScript.
//!
//! # Detected patterns
//!
//! ## Standard pattern (no effects emitted)
//! ```typescript
//! import { appTasks } from "@ohos/hvigor-ohos-plugin";
//! export default { system: appTasks, plugins: [] };
//! ```
//!
//! ## Non-standard: custom plugins in array
//! ```typescript
//! plugins: [entryPlugin]   // → CustomTask effect
//! ```
//!
//! ## compilePluginFile / compilePluginPath overrides
//! ```typescript
//! target.getCompileArkTsOption().compilePluginPath = "./my-plugin.js";
//! // → CompilePluginFile effect
//! ```
//!
//! ## BuildOption overrides
//! ```typescript
//! arkOptions: { obfuscation: { enable: true } }
//! // → BuildOptionOverride effect
//! ```
//!
//! # Upstream correspondence
//!
//! Replaces the JS bridge in `hvigorfile_runtime.ts` for the declarative
//! subset of hvigor script effects.

use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ObjectExpression, ObjectPropertyKind, PropertyKey, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::operator::UnaryOperator;

use std::fmt;
use std::path::{Path, PathBuf};

use arkdown_path::AbsPath;

use crate::build_script::effects::{
    BuildOptionOverrideEffect, BuildScriptEffect, CustomTaskEffect,
};
use crate::build_script::snapshot::{BuildScriptFingerprint, BuildScriptKind, BuildScriptSnapshot};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur while parsing a build script.
#[derive(Debug, thiserror::Error)]
pub enum ParseBuildScriptError {
    /// The script file could not be read.
    #[error("failed to read build script {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The script file name does not match a known build script kind.
    #[error("unrecognized build script file name: {0}")]
    UnrecognizedKind(String),

    /// OXC could not parse the build script.
    #[error("failed to parse build script {path:?}: {message}")]
    Parse { path: PathBuf, message: String },

    /// A config override requires JavaScript execution.
    #[error(
        "build script {path:?} uses a dynamic expression at {option_path}; use the Node package runtime or replace it with a literal object"
    )]
    DynamicOverride { path: PathBuf, option_path: String },
}

// ---------------------------------------------------------------------------
// BuildScriptParser
// ---------------------------------------------------------------------------

/// OXC-backed parser for `hvigorfile.ts` and `hvigorconfig.ts`.
pub struct BuildScriptParser;

impl BuildScriptParser {
    /// Parse a `hvigorfile.ts` or `hvigorconfig.ts` and return a
    /// [`BuildScriptSnapshot`] containing the declarative effects extracted by
    /// OXC static analysis.
    pub fn parse(path: &AbsPath) -> Result<BuildScriptSnapshot, ParseBuildScriptError> {
        Self::parse_scoped(path, true, None)
    }

    /// Parse a project-level build script.
    pub fn parse_project(path: &AbsPath) -> Result<BuildScriptSnapshot, ParseBuildScriptError> {
        Self::parse_scoped(path, false, None)
    }

    /// Parse a module-level build script.
    pub fn parse_module(
        path: &AbsPath,
        module_name: &str,
    ) -> Result<BuildScriptSnapshot, ParseBuildScriptError> {
        Self::parse_scoped(path, true, Some(module_name.to_string()))
    }

    fn parse_scoped(
        path: &AbsPath,
        is_module_level: bool,
        module_name: Option<String>,
    ) -> Result<BuildScriptSnapshot, ParseBuildScriptError> {
        let file_name = path
            .as_path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let kind = match file_name {
            "hvigorfile.ts" => BuildScriptKind::HvigorFile,
            "hvigorconfig.ts" => BuildScriptKind::HvigorConfig,
            other => return Err(ParseBuildScriptError::UnrecognizedKind(other.to_owned())),
        };

        let bytes = std::fs::read(path.as_path()).map_err(|source| ParseBuildScriptError::Io {
            path: path.as_path().to_path_buf(),
            source,
        })?;

        let fingerprint = BuildScriptFingerprint::from_bytes(&bytes);
        let source = String::from_utf8_lossy(&bytes);

        // Walk the AST to extract declarative effects.
        let effects = Self::extract_effects(&source, path.as_path(), kind)?;

        Ok(BuildScriptSnapshot {
            script_path: path.clone(),
            kind,
            is_module_level,
            module_name,
            fingerprint,
            effects,
        })
    }

    fn extract_effects(
        source: &str,
        script_path: &Path,
        _kind: BuildScriptKind,
    ) -> Result<Vec<BuildScriptEffect>, ParseBuildScriptError> {
        let allocator = Allocator::default();
        let source_type = SourceType::ts().with_module(true);
        let parsed = Parser::new(&allocator, source, source_type).parse();

        if !parsed.diagnostics.is_empty() {
            return Err(ParseBuildScriptError::Parse {
                path: script_path.to_path_buf(),
                message: parsed.diagnostics[0].to_string(),
            });
        }

        let mut visitor = ScriptVisitor {
            script_path,
            hvigor_plugin_imports: vec![],
            effects: vec![],
            dynamic_override_path: None,
        };

        visitor.visit_program(&parsed.program.body);
        if let Some(option_path) = visitor.dynamic_override_path {
            return Err(ParseBuildScriptError::DynamicOverride {
                path: script_path.to_path_buf(),
                option_path,
            });
        }
        Ok(visitor.effects)
    }
}

// ---------------------------------------------------------------------------
// Visitor state
// ---------------------------------------------------------------------------

struct ScriptVisitor<'src> {
    script_path: &'src Path,
    /// Names imported from `"@ohos/hvigor-ohos-plugin"` (e.g. `appTasks`).
    hvigor_plugin_imports: Vec<String>,
    effects: Vec<BuildScriptEffect>,
    dynamic_override_path: Option<String>,
}

impl<'src> ScriptVisitor<'src> {
    fn visit_program(&mut self, stmts: &[Statement<'_>]) {
        for stmt in stmts {
            self.visit_statement(stmt);
        }
    }

    fn visit_statement(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::ImportDeclaration(import) => {
                self.visit_import(import);
            }
            Statement::ExportDefaultDeclaration(export) => {
                self.visit_export_default(export);
            }
            Statement::ExpressionStatement(expr_stmt) => {
                self.visit_expression_for_effects(&expr_stmt.expression);
            }
            _ => {}
        }
    }

    // ── Import analysis ────────────────────────────────────────────────────

    fn visit_import(&mut self, import: &oxc_ast::ast::ImportDeclaration<'_>) {
        if import.source.value.as_str() != "@ohos/hvigor-ohos-plugin" {
            return;
        }
        // Collect all named imports from the hvigor plugin package.
        if let Some(specifiers) = &import.specifiers {
            for spec in specifiers {
                if let oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(s) = spec {
                    self.hvigor_plugin_imports
                        .push(s.local.name.as_str().to_owned());
                }
            }
        }
    }

    // ── Default export analysis ────────────────────────────────────────────

    fn visit_export_default(&mut self, export: &oxc_ast::ast::ExportDefaultDeclaration<'_>) {
        let expr = match export.declaration.as_expression() {
            Some(e) => e,
            None => return,
        };

        let obj = match expr {
            Expression::ObjectExpression(o) => o,
            _ => return,
        };

        for prop in &obj.properties {
            let ObjectPropertyKind::ObjectProperty(p) = prop else {
                continue;
            };
            let key_name = match &p.key {
                PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                _ => continue,
            };

            if key_name == "plugins" {
                self.analyze_plugins_property(&p.value);
            } else if key_name == "config" {
                self.analyze_config_property(&p.value);
            }
            // We don't need to analyze `system` — it's always a reference to
            // one of the imported standard tasks (appTasks/hapTasks/etc.).
        }
    }

    fn analyze_config_property(&mut self, value: &Expression<'_>) {
        let Expression::ObjectExpression(config) = value else {
            self.dynamic_override_path = Some("config".to_string());
            return;
        };
        let Some(ohos) = self.object_property(config, "ohos") else {
            return;
        };
        let Expression::ObjectExpression(ohos) = ohos else {
            self.dynamic_override_path = Some("config.ohos".to_string());
            return;
        };
        let Some(overrides) = self.object_property(ohos, "overrides") else {
            return;
        };
        match self.static_json_value(overrides, "config.ohos.overrides") {
            Ok(value) => self.effects.push(BuildScriptEffect::OhosOverrides(value)),
            Err(option_path) => self.dynamic_override_path = Some(option_path),
        }
    }

    fn object_property<'a>(
        &self,
        object: &'a ObjectExpression<'a>,
        name: &str,
    ) -> Option<&'a Expression<'a>> {
        object.properties.iter().rev().find_map(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return None;
            };
            (self.property_name(&property.key).as_deref() == Some(name)).then_some(&property.value)
        })
    }

    fn property_name(&self, key: &PropertyKey<'_>) -> Option<String> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
            PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        }
    }

    fn static_json_value(
        &self,
        expression: &Expression<'_>,
        option_path: &str,
    ) -> Result<serde_json::Value, String> {
        match expression {
            Expression::ObjectExpression(object) => {
                let mut value = serde_json::Map::new();
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return Err(option_path.to_string());
                    };
                    let Some(name) = self.property_name(&property.key) else {
                        return Err(option_path.to_string());
                    };
                    value.insert(
                        name.clone(),
                        self.static_json_value(&property.value, &format!("{option_path}.{name}"))?,
                    );
                }
                Ok(serde_json::Value::Object(value))
            }
            Expression::ArrayExpression(array) => {
                let mut value = Vec::with_capacity(array.elements.len());
                for (index, element) in array.elements.iter().enumerate() {
                    let Some(element) = element.as_expression() else {
                        return Err(format!("{option_path}[{index}]"));
                    };
                    value
                        .push(self.static_json_value(element, &format!("{option_path}[{index}]"))?);
                }
                Ok(serde_json::Value::Array(value))
            }
            Expression::StringLiteral(literal) => {
                Ok(serde_json::Value::String(literal.value.to_string()))
            }
            Expression::NumericLiteral(literal) => self
                .json_number(literal.value)
                .map(serde_json::Value::Number)
                .ok_or_else(|| option_path.to_string()),
            Expression::BooleanLiteral(literal) => Ok(serde_json::Value::Bool(literal.value)),
            Expression::NullLiteral(_) => Ok(serde_json::Value::Null),
            Expression::UnaryExpression(unary) => {
                let Expression::NumericLiteral(literal) = &unary.argument else {
                    return Err(option_path.to_string());
                };
                let value = match unary.operator {
                    UnaryOperator::UnaryPlus => literal.value,
                    UnaryOperator::UnaryNegation => -literal.value,
                    _ => return Err(option_path.to_string()),
                };
                self.json_number(value)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| option_path.to_string())
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.static_json_value(&parenthesized.expression, option_path)
            }
            _ => Err(option_path.to_string()),
        }
    }

    fn json_number(&self, value: f64) -> Option<serde_json::Number> {
        if value.fract() == 0.0 && value >= 0.0 && value <= u64::MAX as f64 {
            return Some(serde_json::Number::from(value as u64));
        }
        if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
            return Some(serde_json::Number::from(value as i64));
        }
        serde_json::Number::from_f64(value)
    }

    fn analyze_plugins_property(&mut self, value: &Expression<'_>) {
        let arr = match value {
            Expression::ArrayExpression(a) => a,
            _ => {
                // Non-array value — treat as custom (unsupported).
                self.effects
                    .push(BuildScriptEffect::CustomTask(CustomTaskEffect {
                        task_name: "unknown_plugin_export".to_string(),
                        module_name: None,
                        action_names: vec![],
                        depends_on: vec![],
                    }));
                return;
            }
        };

        if arr.elements.is_empty() {
            // Standard empty plugins array — no effects needed.
            return;
        }

        // Non-empty plugins array — custom plugins are registered.
        for element in &arr.elements {
            let name = match element {
                oxc_ast::ast::ArrayExpressionElement::SpreadElement(_) => {
                    PluginExpressionName::spread()
                }
                oxc_ast::ast::ArrayExpressionElement::Elision(_) => continue,
                other => {
                    if let Some(expr) = other.as_expression() {
                        PluginExpressionName::from_expression(expr)
                    } else {
                        continue;
                    }
                }
            };
            self.effects
                .push(BuildScriptEffect::CustomTask(CustomTaskEffect {
                    task_name: name.to_string(),
                    module_name: None,
                    action_names: vec![],
                    depends_on: vec![],
                }));
        }
    }

    // ── Expression side-effect scanning ───────────────────────────────────

    /// Walk expressions looking for `compilePluginPath`/`compilePluginFile`
    /// assignments, which indicate a custom compile plugin is being configured.
    fn visit_expression_for_effects(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::CallExpression(call) => {
                // context.targets((target) => { target.getCompileArkTsOption()... })
                for arg in &call.arguments {
                    if let Some(e) = arg.as_expression() {
                        self.visit_expression_for_effects(e);
                    }
                }
            }
            Expression::ArrowFunctionExpression(arrow) => {
                if let oxc_ast::ast::ArrowFunctionBody::FunctionBody(body) = &arrow.body {
                    self.visit_program(&body.statements);
                }
            }
            Expression::AssignmentExpression(assign) => {
                // target.getCompileArkTsOption().compilePluginPath = "./my.js"
                if let Expression::StringLiteral(val) = &assign.right {
                    if let oxc_ast::ast::AssignmentTarget::StaticMemberExpression(member) =
                        &assign.left
                    {
                        let prop = member.property.name.as_str();
                        if matches!(prop, "compilePluginPath" | "compilePluginFile") {
                            let plugin_path = self
                                .script_path
                                .parent()
                                .unwrap_or(std::path::Path::new("."))
                                .join(val.value.as_str());
                            if let Ok(abs) = arkdown_path::AbsPath::from_absolute(plugin_path) {
                                self.effects.push(BuildScriptEffect::CompilePluginFile(abs));
                            } else {
                                self.effects.push(BuildScriptEffect::BuildOptionOverride(
                                    BuildOptionOverrideEffect {
                                        option_path: prop.to_string(),
                                        value: val.value.as_str().to_owned(),
                                    },
                                ));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct PluginExpressionName {
    kind: PluginExpressionNameKind,
}

impl PluginExpressionName {
    const CALL_EXPR: &'static str = "call_expr";
    const SPREAD_PLUGIN: &'static str = "spread_plugin";
    const UNKNOWN: &'static str = "unknown";

    fn spread() -> Self {
        Self::literal(Self::SPREAD_PLUGIN)
    }

    fn from_expression(expr: &Expression<'_>) -> Self {
        match expr {
            Expression::Identifier(id) => Self::identifier(id.name.as_str()),
            Expression::CallExpression(call) => Self::from_call_callee(&call.callee),
            _ => Self::literal(Self::UNKNOWN),
        }
    }

    fn from_call_callee(callee: &Expression<'_>) -> Self {
        match callee {
            Expression::Identifier(id) => Self::call(Self::identifier(id.name.as_str())),
            Expression::StaticMemberExpression(member) => Self::member_call(
                Self::from_expression(&member.object),
                member.property.name.as_str(),
            ),
            _ => Self::literal(Self::CALL_EXPR),
        }
    }

    fn identifier(name: &str) -> Self {
        Self {
            kind: PluginExpressionNameKind::Identifier(name.to_string()),
        }
    }

    fn literal(name: &str) -> Self {
        Self {
            kind: PluginExpressionNameKind::Literal(name.to_string()),
        }
    }

    fn call(callee: Self) -> Self {
        Self {
            kind: PluginExpressionNameKind::Call(Box::new(callee)),
        }
    }

    fn member_call(object: Self, property: &str) -> Self {
        Self {
            kind: PluginExpressionNameKind::MemberCall {
                object: Box::new(object),
                property: property.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone)]
enum PluginExpressionNameKind {
    Identifier(String),
    Literal(String),
    Call(Box<PluginExpressionName>),
    MemberCall {
        object: Box<PluginExpressionName>,
        property: String,
    },
}

impl fmt::Display for PluginExpressionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PluginExpressionNameKind::Identifier(name)
            | PluginExpressionNameKind::Literal(name) => f.write_str(name),
            PluginExpressionNameKind::Call(callee) => write!(f, "{callee}()"),
            PluginExpressionNameKind::MemberCall { object, property } => {
                write!(f, "{object}.{property}()")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_literal_project_ohos_overrides() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("hvigorfile.ts");
        std::fs::write(
            &path,
            r#"
                import { appTasks } from "@ohos/hvigor-ohos-plugin";
                export default {
                    system: appTasks,
                    plugins: [],
                    config: {
                        ohos: {
                            overrides: {
                                appOpt: {
                                    vendor: "example",
                                    versionCode: 200,
                                    bundleName: "com.example.script"
                                }
                            }
                        }
                    }
                };
            "#,
        )
        .unwrap();
        let path = AbsPath::from_absolute(path).unwrap();

        let snapshot = BuildScriptParser::parse_project(&path).unwrap();
        assert!(!snapshot.is_module_level);
        let overrides = snapshot.ohos_overrides().unwrap();
        assert_eq!(overrides["appOpt"]["versionCode"], 200);
        assert_eq!(overrides["appOpt"]["bundleName"], "com.example.script");
    }

    #[test]
    fn rejects_dynamic_override_in_pure_rust_parser() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("hvigorfile.ts");
        std::fs::write(
            &path,
            r#"
                export default {
                    config: {
                        ohos: {
                            overrides: {
                                appOpt: { versionCode: getVersionCode() }
                            }
                        }
                    }
                };
            "#,
        )
        .unwrap();
        let path = AbsPath::from_absolute(path).unwrap();

        let error = BuildScriptParser::parse_project(&path).unwrap_err();
        assert!(matches!(
            error,
            ParseBuildScriptError::DynamicOverride { ref option_path, .. }
                if option_path == "config.ohos.overrides.appOpt.versionCode"
        ));
    }
}
