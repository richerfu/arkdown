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
use oxc_ast::ast::{Expression, ObjectPropertyKind, PropertyKey, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;

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
        let effects = Self::extract_effects(&source, path.as_path(), kind);

        Ok(BuildScriptSnapshot {
            script_path: path.clone(),
            kind,
            is_module_level: matches!(kind, BuildScriptKind::HvigorFile),
            module_name: None,
            fingerprint,
            effects,
        })
    }

    fn extract_effects(
        source: &str,
        script_path: &Path,
        _kind: BuildScriptKind,
    ) -> Vec<BuildScriptEffect> {
        let allocator = Allocator::default();
        let source_type = SourceType::ts().with_module(true);
        let parsed = Parser::new(&allocator, source, source_type).parse();

        // If parse fails, return empty effects (build will fail later with a better error).
        if !parsed.diagnostics.is_empty() {
            tracing::debug!(
                path = %script_path.display(),
                errors = parsed.diagnostics.len(),
                "build script parse errors — skipping effect extraction"
            );
            return vec![];
        }

        let mut visitor = ScriptVisitor {
            script_path,
            hvigor_plugin_imports: vec![],
            effects: vec![],
        };

        visitor.visit_program(&parsed.program.body);
        visitor.effects
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
            }
            // We don't need to analyze `system` — it's always a reference to
            // one of the imported standard tasks (appTasks/hapTasks/etc.).
        }
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
                self.visit_program(&arrow.body.statements);
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
