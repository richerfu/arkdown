//! External compile plugin support — declarative subset.
//!
//! Aligns with hvigor's `compilePluginPath` mechanism via the same interface
//! as Rolldown's plugin system.
//!
//! # Architecture (following Rolldown)
//!
//! ```text
//! External JS compile plugin file
//!         │
//!         ├── OXC static analysis ──────────────────────────────────── P1
//!         │   Detect declarative hook patterns:
//!         │   · resolveId: "id-a" → "id-b"    ──→ ResolveIdAlias
//!         │   · load: "\0virtual" → content    ──→ VirtualModule
//!         │
//!         │   Each PluginHookDecl becomes a Rust `rolldown_plugin::Plugin`
//!         │   implementation (DeclarativePlugin) inserted at the configured
//!         │   `order` position in the PluginFactory list.
//!         │
//!         ├── Embedded QuickJS (future — P2)
//!         │   ExternalJsPlugin implements rolldown_plugin::Plugin by calling
//!         │   into a rquickjs runtime for arbitrary hook code.
//!         │
//!         └── Unsupported → typed UnsupportedPlugin error
//!             Lists which patterns were found that exceeded declarative scope.
//! ```
//!
//! # Plugin hook contract (from hvigor `validatePlugin`)
//!
//! An external compile plugin is a JS default export with:
//! ```typescript
//! export default {
//!   name: string,                              // required
//!   order?: "first" | "last" | Function,       // default: "last"
//!   buildStart?():    void,
//!   resolveId?(id, importer): string | null,
//!   load?(id):        string | null,
//!   transform?(code, id): string | null,
//!   shouldInvalidCache?(cached): boolean,
//!   moduleParsed?(module): void,
//!   beforeBuildEnd?(output): void,
//!   buildEnd?(error): void,
//! }
//! ```
//!
//! # Declarative patterns supported in P1
//!
//! A plugin is "declarative" when every hook implementation is a pure static
//! function with no side effects — specifically:
//!
//! - `resolveId`: only `if (id === "literal") return "literal";` patterns
//! - `load`: only `if (id === "literal") return "literal string content";`
//! - `transform`: executed by the Node.js plugin runtime, not reimplemented as
//!   Rust source-text substitution.
//!
//! Any other pattern causes the analysis to return `UnsupportedPlugin`.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use oxc_syntax::operator::BinaryOperator;
use rolldown_plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
    HookResolveIdReturn, HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};
use rustc_hash::FxHashMap;

// ---------------------------------------------------------------------------
// Plugin order
// ---------------------------------------------------------------------------

/// Insertion order for an external plugin in the built-in pipeline.
///
/// Mirrors hvigor's `validatePlugin` `order` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginOrder {
    /// Insert before all built-in plugins.
    First,
    /// Insert after all built-in plugins (default).
    Last,
}

impl Default for PluginOrder {
    fn default() -> Self {
        Self::Last
    }
}

// ---------------------------------------------------------------------------
// Declarative hook declarations
// ---------------------------------------------------------------------------

/// A single declarative hook extracted from an external plugin file by OXC.
///
/// Each variant corresponds to a statically-analyzable Rolldown hook pattern.
/// Non-declarative hooks yield [`UnsupportedPlugin`] during analysis.
#[derive(Debug, Clone)]
pub enum PluginHookDecl {
    /// `resolveId` hook: rewrite a static module specifier to another.
    ///
    /// # Upstream pattern
    /// ```javascript
    /// resolveId(id) { if (id === "from") return "to"; return null; }
    /// ```
    ResolveIdAlias {
        /// The import specifier to match (exact string equality).
        from: String,
        /// The specifier to return instead.
        to: String,
    },

    /// `load` hook: return static content for a virtual module.
    ///
    /// # Upstream pattern
    /// ```javascript
    /// load(id) { if (id === "\0virtual-id") return "export default 42;"; }
    /// ```
    VirtualModule {
        /// The module id (usually prefixed with `\0`).
        id: String,
        /// The static source content to return.
        content: String,
    },
}

// ---------------------------------------------------------------------------
// Analysis result
// ---------------------------------------------------------------------------

/// Result of statically analyzing an external compile plugin file.
#[derive(Debug)]
pub enum CompilePluginAnalysis {
    /// The plugin only uses declarative patterns that can run without a JS runtime.
    Declarative {
        /// Plugin name (from the `name:` property).
        name: String,
        /// Insertion order in the pipeline.
        order: PluginOrder,
        /// The extracted hook declarations.
        hooks: Vec<PluginHookDecl>,
    },

    /// The plugin contains patterns that require a JS runtime to execute.
    Unsupported {
        /// The plugin name if extractable, or the file name.
        name: String,
        /// Reasons why the plugin cannot be statically analyzed.
        reasons: Vec<UnsupportedReason>,
    },
}

/// Reason why a plugin's hook could not be analyzed statically.
#[derive(Debug, Clone)]
pub struct UnsupportedReason {
    /// The hook name (e.g., `"transform"`).
    pub hook: String,
    /// Human-readable description of the unsupported pattern.
    pub description: String,
}

// ---------------------------------------------------------------------------
// OXC-based analysis
// ---------------------------------------------------------------------------

struct CompilePluginProtocol;

impl CompilePluginProtocol {
    const PROPERTY_NAME: &'static str = "name";
    const PROPERTY_ORDER: &'static str = "order";
    const ORDER_FIRST: &'static str = "first";
    const ORDER_LAST: &'static str = "last";
    const HOOK_PARSE: &'static str = "parse";
    const HOOK_EXPORT: &'static str = "export";
    const HOOK_RESOLVE_ID: &'static str = "resolveId";
    const HOOK_LOAD: &'static str = "load";
    const HOOK_TRANSFORM: &'static str = "transform";
    const HOOK_BUILD_START: &'static str = "buildStart";
    const HOOK_BUILD_END: &'static str = "buildEnd";
    const HOOK_SHOULD_INVALID_CACHE: &'static str = "shouldInvalidCache";
    const HOOK_MODULE_PARSED: &'static str = "moduleParsed";
    const HOOK_BEFORE_BUILD_END: &'static str = "beforeBuildEnd";

    fn order_from_literal(value: &str) -> Option<PluginOrder> {
        match value {
            Self::ORDER_FIRST => Some(PluginOrder::First),
            Self::ORDER_LAST => Some(PluginOrder::Last),
            _ => None,
        }
    }

    fn is_declarative_hook(key: &str) -> bool {
        matches!(key, Self::HOOK_RESOLVE_ID | Self::HOOK_LOAD)
    }

    fn is_noop_hook(key: &str) -> bool {
        matches!(
            key,
            Self::HOOK_BUILD_START
                | Self::HOOK_BUILD_END
                | Self::HOOK_SHOULD_INVALID_CACHE
                | Self::HOOK_MODULE_PARSED
                | Self::HOOK_BEFORE_BUILD_END
        )
    }
}

struct CompilePluginAnalyzer<'a> {
    source: &'a str,
    default_name: &'a str,
}

struct CompilePluginFile<'a> {
    path: &'a Path,
}

impl<'a> CompilePluginFile<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn analyze(&self) -> Result<CompilePluginAnalysis> {
        let source = std::fs::read_to_string(self.path).map_err(|e| {
            anyhow::anyhow!(
                "failed to read compile plugin {}: {}",
                self.path.display(),
                e
            )
        })?;

        let file_name = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(CompilePluginAnalyzer::new(&source, &file_name).analyze())
    }
}

impl<'a> CompilePluginAnalyzer<'a> {
    fn new(source: &'a str, default_name: &'a str) -> Self {
        Self {
            source,
            default_name,
        }
    }

    fn analyze(&self) -> CompilePluginAnalysis {
        use oxc_allocator::Allocator;
        use oxc_parser::Parser;
        use oxc_span::SourceType;

        let allocator = Allocator::default();
        let source_type = SourceType::ts().with_module(true);
        let parsed = Parser::new(&allocator, self.source, source_type).parse();

        if !parsed.diagnostics.is_empty() {
            return CompilePluginAnalysis::Unsupported {
                name: self.default_name.to_string(),
                reasons: vec![UnsupportedReason {
                    hook: CompilePluginProtocol::HOOK_PARSE.to_string(),
                    description: format!(
                        "{} parse errors in plugin file",
                        parsed.diagnostics.len()
                    ),
                }],
            };
        }

        let Some(obj) = self.default_export_object(&parsed.program.body) else {
            return CompilePluginAnalysis::Unsupported {
                name: self.default_name.to_string(),
                reasons: vec![UnsupportedReason {
                    hook: CompilePluginProtocol::HOOK_EXPORT.to_string(),
                    description:
                        "plugin file does not have a default export with an object literal"
                            .to_string(),
                }],
            };
        };

        self.analyze_properties(obj)
    }

    fn default_export_object<'b>(
        &self,
        body: &'b oxc_allocator::Vec<'b, oxc_ast::ast::Statement<'b>>,
    ) -> Option<&'b oxc_ast::ast::ObjectExpression<'b>> {
        use oxc_ast::ast::{Expression, Statement};

        body.iter()
            .find_map(|stmt| {
                let Statement::ExportDefaultDeclaration(decl) = stmt else {
                    return None;
                };
                let Some(Expression::ObjectExpression(obj)) = decl.declaration.as_expression()
                else {
                    return None;
                };
                Some(obj)
            })
            .map(|obj| &**obj)
    }

    fn analyze_properties(
        &self,
        obj: &oxc_ast::ast::ObjectExpression<'_>,
    ) -> CompilePluginAnalysis {
        use oxc_ast::ast::{Expression, ObjectPropertyKind, PropertyKey};

        let mut name = self.default_name.to_string();
        let mut order = PluginOrder::Last;
        let mut hooks: Vec<PluginHookDecl> = Vec::new();
        let mut unsupported: Vec<UnsupportedReason> = Vec::new();

        for prop_kind in &obj.properties {
            let ObjectPropertyKind::ObjectProperty(prop) = prop_kind else {
                continue;
            };
            let key = match &prop.key {
                PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                _ => continue,
            };

            match key {
                CompilePluginProtocol::PROPERTY_NAME => {
                    if let Expression::StringLiteral(s) = &prop.value {
                        name = s.value.as_str().to_owned();
                    }
                }
                CompilePluginProtocol::PROPERTY_ORDER => {
                    if let Expression::StringLiteral(s) = &prop.value {
                        if let Some(parsed_order) =
                            CompilePluginProtocol::order_from_literal(s.value.as_str())
                        {
                            order = parsed_order;
                        } else {
                            unsupported.push(UnsupportedReason {
                                hook: CompilePluginProtocol::PROPERTY_ORDER.to_string(),
                                description: format!(
                                    "order value '{}' is not 'first' or 'last'",
                                    s.value.as_str()
                                ),
                            });
                        }
                    } else {
                        unsupported.push(UnsupportedReason {
                            hook: CompilePluginProtocol::PROPERTY_ORDER.to_string(),
                            description: "dynamic order function not supported".to_string(),
                        });
                    }
                }
                hook_name if CompilePluginProtocol::is_declarative_hook(hook_name) => {
                    match HookDeclExtractor::new(hook_name).extract(&prop.value) {
                        Ok(Some(decl)) => hooks.push(decl),
                        Ok(None) => {}
                        Err(reason) => unsupported.push(reason),
                    }
                }
                CompilePluginProtocol::HOOK_TRANSFORM => {
                    unsupported.push(UnsupportedReason {
                        hook: CompilePluginProtocol::HOOK_TRANSFORM.to_string(),
                        description: "transform hook requires the JavaScript plugin runtime"
                            .to_string(),
                    });
                }
                hook_name if CompilePluginProtocol::is_noop_hook(hook_name) => {}
                other => {
                    unsupported.push(UnsupportedReason {
                        hook: other.to_string(),
                        description: format!("unknown hook '{other}'"),
                    });
                }
            }
        }

        if !unsupported.is_empty() {
            CompilePluginAnalysis::Unsupported {
                name,
                reasons: unsupported,
            }
        } else {
            CompilePluginAnalysis::Declarative { name, order, hooks }
        }
    }
}

// ---------------------------------------------------------------------------
// Hook pattern extraction
// ---------------------------------------------------------------------------

struct HookDeclExtractor<'a> {
    hook_name: &'a str,
}

impl<'a> HookDeclExtractor<'a> {
    fn new(hook_name: &'a str) -> Self {
        Self { hook_name }
    }

    fn extract(
        &self,
        value: &oxc_ast::ast::Expression<'_>,
    ) -> std::result::Result<Option<PluginHookDecl>, UnsupportedReason> {
        use oxc_ast::ast::{Expression, Statement};

        let body_stmts: Vec<&Statement<'_>> = match value {
            Expression::FunctionExpression(func) => func
                .body
                .as_ref()
                .map(|b| b.statements.iter().collect())
                .unwrap_or_default(),
            Expression::ArrowFunctionExpression(arrow) => arrow.body.statements.iter().collect(),
            _ => {
                return Err(UnsupportedReason {
                    hook: self.hook_name.to_string(),
                    description: format!("'{}' is not a function expression", self.hook_name),
                });
            }
        };

        match self.hook_name {
            CompilePluginProtocol::HOOK_RESOLVE_ID => self.extract_resolve_id_decl(body_stmts),
            CompilePluginProtocol::HOOK_LOAD => self.extract_load_decl(body_stmts),
            _ => Ok(None),
        }
    }

    /// Detect: `if (id === "from") return "to"; return null;`
    fn extract_resolve_id_decl(
        &self,
        stmts: Vec<&oxc_ast::ast::Statement<'_>>,
    ) -> std::result::Result<Option<PluginHookDecl>, UnsupportedReason> {
        use oxc_ast::ast::{Expression, Statement};

        let if_stmt = stmts.iter().find_map(|s| {
            if let Statement::IfStatement(i) = s {
                Some(i)
            } else {
                None
            }
        });

        let Some(if_stmt) = if_stmt else {
            return Ok(None);
        };

        let (lhs, rhs) = match &if_stmt.test {
            Expression::BinaryExpression(bin) if bin.operator == BinaryOperator::StrictEquality => {
                (&bin.left, &bin.right)
            }
            _ => {
                return Err(UnsupportedReason {
                    hook: self.hook_name.to_string(),
                    description: "condition is not a strict equality check".to_string(),
                });
            }
        };

        let from = match (lhs, rhs) {
            (Expression::Identifier(_), Expression::StringLiteral(s)) => {
                s.value.as_str().to_owned()
            }
            (Expression::StringLiteral(s), Expression::Identifier(_)) => {
                s.value.as_str().to_owned()
            }
            _ => {
                return Err(UnsupportedReason {
                    hook: self.hook_name.to_string(),
                    description: "condition is not `id === \"literal\"`".to_string(),
                });
            }
        };

        let to = self.return_string_from_statement(&if_stmt.consequent)?;
        Ok(Some(PluginHookDecl::ResolveIdAlias { from, to }))
    }

    /// Detect: `if (id === "\0virtual") return "export default 42;";`
    fn extract_load_decl(
        &self,
        stmts: Vec<&oxc_ast::ast::Statement<'_>>,
    ) -> std::result::Result<Option<PluginHookDecl>, UnsupportedReason> {
        match self.extract_resolve_id_decl(stmts) {
            Ok(Some(PluginHookDecl::ResolveIdAlias { from, to })) => {
                Ok(Some(PluginHookDecl::VirtualModule {
                    id: from,
                    content: to,
                }))
            }
            Ok(_) => Ok(None),
            Err(reason) => Err(reason),
        }
    }

    fn return_string_from_statement(
        &self,
        statement: &oxc_ast::ast::Statement<'_>,
    ) -> std::result::Result<String, UnsupportedReason> {
        use oxc_ast::ast::{Expression, Statement};

        match statement {
            Statement::ReturnStatement(ret) => match ret.argument.as_ref() {
                Some(Expression::StringLiteral(s)) => Ok(s.value.as_str().to_owned()),
                _ => Err(self.unsupported_return_value()),
            },
            Statement::BlockStatement(block) => {
                let ret = block.body.iter().find_map(|s| {
                    if let Statement::ReturnStatement(r) = s {
                        Some(r)
                    } else {
                        None
                    }
                });
                match ret.and_then(|r| r.argument.as_ref()) {
                    Some(Expression::StringLiteral(s)) => Ok(s.value.as_str().to_owned()),
                    _ => Err(self.unsupported_return_value()),
                }
            }
            _ => Err(UnsupportedReason {
                hook: self.hook_name.to_string(),
                description: "consequent is not a return or block statement".to_string(),
            }),
        }
    }

    fn unsupported_return_value(&self) -> UnsupportedReason {
        UnsupportedReason {
            hook: self.hook_name.to_string(),
            description: "return value is not a string literal".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// DeclarativePlugin: Rolldown Plugin impl
// ---------------------------------------------------------------------------

/// A Rolldown `Plugin` implementation backed by statically-extracted hook
/// declarations.
///
/// This is the bridge from OXC analysis to the Rolldown plugin pipeline:
/// each [`PluginHookDecl`] becomes an async Rolldown hook implementation.
pub struct DeclarativePlugin {
    plugin_name: String,
    /// Static alias map built from `ResolveIdAlias` declarations.
    resolve_aliases: Arc<FxHashMap<String, String>>,
    /// Virtual module content map built from `VirtualModule` declarations.
    virtual_modules: Arc<FxHashMap<String, String>>,
}

impl DeclarativePlugin {
    /// Build a `DeclarativePlugin` from a list of extracted hook declarations.
    pub fn from_decls(name: String, hooks: Vec<PluginHookDecl>) -> Arc<Self> {
        let mut resolve_aliases = FxHashMap::default();
        let mut virtual_modules = FxHashMap::default();

        for decl in hooks {
            match decl {
                PluginHookDecl::ResolveIdAlias { from, to } => {
                    resolve_aliases.insert(from, to);
                }
                PluginHookDecl::VirtualModule { id, content } => {
                    virtual_modules.insert(id, content);
                }
            }
        }

        Arc::new(Self {
            plugin_name: name,
            resolve_aliases: Arc::new(resolve_aliases),
            virtual_modules: Arc::new(virtual_modules),
        })
    }
}

impl std::fmt::Debug for DeclarativePlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeclarativePlugin")
            .field("name", &self.plugin_name)
            .field("resolve_aliases", &self.resolve_aliases.len())
            .field("virtual_modules", &self.virtual_modules.len())
            .finish()
    }
}

impl Plugin for DeclarativePlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("declarative-plugin")
    }

    fn register_hook_usage(&self) -> HookUsage {
        let mut usage = HookUsage::empty();
        if !self.resolve_aliases.is_empty() || !self.virtual_modules.is_empty() {
            usage |= HookUsage::ResolveId;
        }
        if !self.virtual_modules.is_empty() {
            usage |= HookUsage::Load;
        }
        usage
    }

    fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
        let specifier = args.specifier.to_owned();
        let resolve_aliases = Arc::clone(&self.resolve_aliases);
        let virtual_modules = Arc::clone(&self.virtual_modules);
        async move {
            if let Some(to) = resolve_aliases.get(specifier.as_str()) {
                return Ok(Some(HookResolveIdOutput {
                    id: to.clone().into(),
                    ..Default::default()
                }));
            }
            if virtual_modules.contains_key(specifier.as_str()) {
                return Ok(Some(HookResolveIdOutput {
                    id: specifier.into(),
                    ..Default::default()
                }));
            }
            Ok(None)
        }
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let id = args.id.to_owned();
        let virtual_modules = Arc::clone(&self.virtual_modules);
        async move {
            if let Some(content) = virtual_modules.get(id.as_str()) {
                return Ok(Some(HookLoadOutput {
                    code: content.clone().into(),
                    ..Default::default()
                }));
            }
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Loader for PluginFactory integration
// ---------------------------------------------------------------------------

/// Loads external compile plugins through the OXC declarative analyzer and the
/// Node.js fallback runtime.
pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    /// Load a compile plugin file.
    ///
    /// Tries declarative OXC analysis first (fast, no subprocess). Falls back
    /// to Node.js subprocess execution for plugins with complex logic.
    ///
    /// Returns `None` only when Node.js is also unavailable or the file is
    /// completely unloadable.
    pub async fn load(
        path: &Path,
        order: PluginOrder,
    ) -> Option<(rolldown_plugin::__inner::SharedPluginable, PluginOrder)> {
        // Fast path: try to satisfy the plugin declaratively via OXC analysis.
        if let Some((plugin, detected_order)) = Self::load_declarative(path) {
            let effective_order = if matches!(order, PluginOrder::First) {
                order
            } else {
                detected_order
            };
            return Some((
                plugin as rolldown_plugin::__inner::SharedPluginable,
                effective_order,
            ));
        }

        // Slow path: fall back to Node.js subprocess for complex plugins.
        tracing::debug!(
            path = %path.display(),
            "plugin exceeds declarative subset — falling back to Node.js subprocess"
        );
        if let Some((plugin, detected_order)) =
            crate::plugins::node_js::NodeJsPluginLoader::load(path, order).await
        {
            let effective_order = if matches!(order, PluginOrder::First) {
                order
            } else {
                detected_order
            };
            return Some((
                plugin as rolldown_plugin::__inner::SharedPluginable,
                effective_order,
            ));
        }

        None
    }

    /// Load and analyze an external compile plugin file using only declarative
    /// OXC analysis (no subprocess). Returns `None` for unsupported plugins.
    pub fn load_declarative(path: &Path) -> Option<(Arc<DeclarativePlugin>, PluginOrder)> {
        match CompilePluginFile::new(path).analyze() {
            Ok(CompilePluginAnalysis::Declarative { name, order, hooks }) => {
                tracing::info!(
                    plugin = %name,
                    path = %path.display(),
                    hooks = hooks.len(),
                    "loaded declarative compile plugin (OXC)"
                );
                Some((DeclarativePlugin::from_decls(name, hooks), order))
            }
            Ok(CompilePluginAnalysis::Unsupported { name, reasons }) => {
                let reason_list: Vec<&str> =
                    reasons.iter().map(|r| r.description.as_str()).collect();
                // Function-type `order` is not supported: hvigor allows
                // `order?: "first" | "last" | Function` but arkdown only
                // supports the string literals. Emit a warning so that build
                // authors are not silently surprised when their plugin is
                // skipped or inserted at the wrong position.
                let has_function_order = reasons
                    .iter()
                    .any(|r| r.hook == "order" && r.description.contains("dynamic order function"));
                if has_function_order {
                    tracing::warn!(
                        plugin = %name,
                        path = %path.display(),
                        "compile plugin specifies a function-type `order`; \
                         only \"first\" and \"last\" string literals are supported — \
                         plugin will be treated as unsupported"
                    );
                } else {
                    tracing::debug!(
                        plugin = %name,
                        path = %path.display(),
                        reasons = ?reason_list,
                        "compile plugin is not declarative — will try Node.js fallback"
                    );
                }
                None
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "failed to analyze compile plugin"
                );
                None
            }
        }
    }
}
