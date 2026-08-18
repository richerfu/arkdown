//! Router-map navigation builder registration.
//!
//! The SDK ETS transformer indexes `routerMap` entries by the physical source
//! filename and appends a guarded `NavigationBuilderRegister` IIFE after that
//! source has been transformed.  Keep the index in the transform plan so the
//! Rolldown hook only performs a lookup and never reads profile artifacts.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::{canonicalize_or_owned, lexical_normalize};
use oxc_allocator::{Allocator, ArenaBox, ArenaVec, Vec as OxcVec};
use oxc_ast::ast::{
    Argument, Expression, FormalParameterKind, FormalParameters, Function, FunctionBody,
    FunctionType, Statement,
};
use oxc_ast::builder::AstBuilder;
use oxc_span::SPAN;
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::error::EtsTransformError;

/// A `routerMap` entry consumed by the ETS transform plan.
///
/// This is deliberately detached from the serialized loader schema: profile
/// processing owns the JSON artifact, while ETS transform owns the source-file
/// to runtime-registration relation used by `process_ui_syntax.js`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationRoute {
    source_path: PathBuf,
    name: String,
    build_function: String,
}

impl NavigationRoute {
    pub fn new(
        source_path: impl Into<PathBuf>,
        name: impl Into<String>,
        build_function: impl Into<String>,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            name: name.into(),
            build_function: build_function.into(),
        }
    }
}

/// Immutable, source-indexed router-map data available to every ETS hook.
///
/// `process_ui_syntax.js::createNavigationInit` receives the router data
/// before file transformation.  Building this index once in the coordinator
/// preserves that lifecycle and keeps hook execution allocation-light.
#[derive(Debug, Clone, Default)]
pub struct NavigationRouteMap {
    routes_by_source: Arc<FxHashMap<PathBuf, Vec<NavigationRoute>>>,
}

impl NavigationRouteMap {
    /// Convert the hvigor-compatible `loader.json.routerMap` values into the
    /// transform-plan representation. Entries without a physical source-file
    /// record cannot be associated with an ETS source and are ignored, just as
    /// the SDK's `routerInfo` lookup cannot reach them.
    pub fn from_loader_entries(entries: &[serde_json::Value]) -> Self {
        NavigationRouteMapCollector::new(entries).collect()
    }

    /// Construct a map from already typed route records.
    ///
    /// Kept public for programmatic users of the transform crate and focused
    /// parity tests; production builds use [`Self::from_loader_entries`].
    pub fn from_routes(routes: impl IntoIterator<Item = NavigationRoute>) -> Self {
        NavigationRouteMapCollector::from_routes(routes).collect()
    }

    pub(crate) fn routes_for_source(&self, source_id: &str) -> Option<&[NavigationRoute]> {
        let path = NavigationRouteSourcePath::normalize(Path::new(source_id));
        self.routes_by_source
            .get(&path)
            .map(|routes| routes.as_slice())
    }
}

struct NavigationRouteSourcePath;

impl NavigationRouteSourcePath {
    /// Resolve aliases such as macOS `/tmp` → `/private/tmp` before Rolldown
    /// begins. This mirrors the source snapshot's canonical index and keeps
    /// Windows drive/UNC spelling in host-native `PathBuf` form.
    fn normalize(path: &Path) -> PathBuf {
        canonicalize_or_owned(&lexical_normalize(path))
    }
}

enum NavigationRouteMapInput<'a> {
    LoaderEntries(&'a [serde_json::Value]),
    Routes(Vec<NavigationRoute>),
}

struct NavigationRouteMapCollector<'a> {
    input: NavigationRouteMapInput<'a>,
}

impl<'a> NavigationRouteMapCollector<'a> {
    fn new(entries: &'a [serde_json::Value]) -> Self {
        Self {
            input: NavigationRouteMapInput::LoaderEntries(entries),
        }
    }

    fn from_routes(routes: impl IntoIterator<Item = NavigationRoute>) -> Self {
        Self {
            input: NavigationRouteMapInput::Routes(routes.into_iter().collect()),
        }
    }

    fn collect(self) -> NavigationRouteMap {
        let routes = match self.input {
            NavigationRouteMapInput::LoaderEntries(entries) => entries
                .iter()
                .filter_map(Self::loader_entry_route)
                .collect(),
            NavigationRouteMapInput::Routes(routes) => routes,
        };
        let mut routes_by_source = FxHashMap::default();
        for mut route in routes {
            let source_path = NavigationRouteSourcePath::normalize(&route.source_path);
            route.source_path = source_path.clone();
            routes_by_source
                .entry(source_path)
                .or_insert_with(Vec::new)
                .push(route);
        }
        NavigationRouteMap {
            routes_by_source: Arc::new(routes_by_source),
        }
    }

    fn loader_entry_route(entry: &serde_json::Value) -> Option<NavigationRoute> {
        let object = entry.as_object()?;
        let source_path = object.get("pageSourceFile")?.as_str()?;
        let name = object.get("name")?.as_str()?;
        let build_function = object.get("buildFunction")?.as_str()?;
        Some(NavigationRoute::new(source_path, name, build_function))
    }
}

/// Deferred generated statements for one source's router-map entries.
#[derive(Debug, Default)]
pub(super) struct NavigationBuilderRegistrationBlock {
    routes: Vec<NavigationRoute>,
}

impl NavigationBuilderRegistrationBlock {
    pub(super) fn for_source(
        routes: Option<&[NavigationRoute]>,
        local_builder_functions: &FxHashSet<String>,
        source_id: &str,
    ) -> Result<Self, EtsTransformError> {
        let routes = routes.unwrap_or_default();
        for route in routes {
            if !local_builder_functions.contains(&route.build_function) {
                // Exact SDK owner: ets-loader/lib/process_ui_syntax.js ::
                // createNavigationInit. It reports an error rather than
                // emitting a registration for a missing local @Builder.
                return Err(EtsTransformError::Fatal {
                    file: source_id.to_string(),
                    message: format!(
                        "The buildFunction '{}' configured in the routerMap json file does not exist.",
                        route.build_function
                    ),
                });
            }
        }
        Ok(Self {
            routes: routes.to_vec(),
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub(super) fn statements<'a>(
        &self,
        ast: &AstBuilder<'a>,
        allocator: &'a Allocator,
    ) -> OxcVec<'a, Statement<'a>> {
        if self.routes.is_empty() {
            return OxcVec::new_in(ast);
        }
        NavigationBuilderRegistrationAstEmitter::new(&self.routes).statements(ast, allocator)
    }
}

struct NavigationBuilderRuntimeName;

impl NavigationBuilderRuntimeName {
    const REGISTER: &'static str = "NavigationBuilderRegister";
    const WRAP_BUILDER: &'static str = "wrapBuilder";
    const FUNCTION: &'static str = "function";
}

struct NavigationBuilderRegistrationAstEmitter<'a> {
    routes: &'a [NavigationRoute],
}

impl<'a> NavigationBuilderRegistrationAstEmitter<'a> {
    fn new(routes: &'a [NavigationRoute]) -> Self {
        Self { routes }
    }

    fn statements<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
    ) -> OxcVec<'b, Statement<'b>> {
        let mut registrations = OxcVec::new_in(ast);
        for route in self.routes {
            registrations.push(self.registration_statement(ast, allocator, route));
        }

        let guard = Expression::new_binary_expression(
            SPAN,
            Expression::new_unary_expression(
                SPAN,
                UnaryOperator::Typeof,
                Expression::new_identifier(SPAN, NavigationBuilderRuntimeName::REGISTER, ast),
                ast,
            ),
            BinaryOperator::StrictEquality,
            Expression::new_string_literal(SPAN, NavigationBuilderRuntimeName::FUNCTION, None, ast),
            ast,
        );
        let guarded_registrations = Statement::new_if_statement(
            SPAN,
            guard,
            Statement::new_block_statement(SPAN, registrations, ast),
            None,
            ast,
        );
        let function = Expression::FunctionExpression(ArenaBox::new_in(
            Function::new(
                SPAN,
                FunctionType::FunctionExpression,
                None,
                false,
                false,
                false,
                None,
                None,
                ArenaBox::new_in(
                    FormalParameters::new(
                        SPAN,
                        FormalParameterKind::FormalParameter,
                        ArenaVec::new_in(ast),
                        None,
                        ast,
                    ),
                    ast,
                ),
                None,
                Some(ArenaBox::new_in(
                    FunctionBody::new(
                        SPAN,
                        ArenaVec::new_in(ast),
                        ArenaVec::from_value_in(guarded_registrations, ast),
                        ast,
                    ),
                    ast,
                )),
                ast,
            ),
            ast,
        ));
        let invoke = Expression::new_call_expression(
            SPAN,
            Expression::new_parenthesized_expression(SPAN, function, ast),
            None,
            ArenaVec::new_in(ast),
            false,
            ast,
        );
        OxcVec::from_value_in(Statement::new_expression_statement(SPAN, invoke, ast), ast)
    }

    fn registration_statement<'b>(
        &self,
        ast: &AstBuilder<'b>,
        allocator: &'b Allocator,
        route: &NavigationRoute,
    ) -> Statement<'b> {
        let wrapped_builder = Expression::new_call_expression(
            SPAN,
            Expression::new_identifier(SPAN, NavigationBuilderRuntimeName::WRAP_BUILDER, ast),
            None,
            ArenaVec::from_value_in(
                Argument::from(Expression::new_identifier(
                    SPAN,
                    allocator.alloc_str(&route.build_function),
                    ast,
                )),
                ast,
            ),
            false,
            ast,
        );
        let registration = Expression::new_call_expression(
            SPAN,
            Expression::new_identifier(SPAN, NavigationBuilderRuntimeName::REGISTER, ast),
            None,
            ArenaVec::from_iter_in(
                [
                    Argument::from(Expression::new_string_literal(
                        SPAN,
                        allocator.alloc_str(&route.name),
                        None,
                        ast,
                    )),
                    Argument::from(wrapped_builder),
                ],
                ast,
            ),
            false,
            ast,
        );
        Statement::new_expression_statement(SPAN, registration, ast)
    }
}
