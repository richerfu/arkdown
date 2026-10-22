use oxc_ast::ast::{ImportDeclarationSpecifier, Program, Statement, TSModuleReference};
use oxc_ast::AstKind;
use oxc_ast_visit::Visit;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

use crate::kit_map::resolve_kit_import;

use super::component_meta::ComponentMetadata;

/// Per-source data written by the ArkUI-X `apiTransform` lifecycle to
/// `module_collection.json` and `component_collection.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossplatformSourceCollection {
    pub modules: Vec<String>,
    pub components: Vec<String>,
}

impl CrossplatformSourceCollection {
    pub(crate) fn from_program(
        program: &Program<'_>,
        module_prefixes: &[Box<str>],
        component_metadata: &ComponentMetadata,
    ) -> Self {
        let mut collection = Self {
            modules: CrossplatformModuleCollector::new(module_prefixes).collect(program),
            components: Vec::new(),
        };
        let mut component_collector =
            CrossplatformComponentCollector::new(component_metadata, &mut collection.components);
        component_collector.visit_program(program);
        collection
    }

    pub(crate) fn modules_from_program(
        program: &Program<'_>,
        module_prefixes: &[Box<str>],
    ) -> Self {
        Self {
            modules: CrossplatformModuleCollector::new(module_prefixes).collect(program),
            components: Vec::new(),
        }
    }

    pub fn merge(&mut self, other: &Self) {
        Self::extend_unique(&mut self.modules, &other.modules);
        Self::extend_unique(&mut self.components, &other.components);
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty() && self.components.is_empty()
    }

    fn extend_unique(target: &mut Vec<String>, values: &[String]) {
        for value in values {
            if !target.contains(value) {
                target.push(value.clone());
            }
        }
    }
}

struct CrossplatformModuleCollector<'a> {
    module_prefixes: &'a [Box<str>],
    modules: Vec<String>,
    seen: FxHashSet<String>,
}

impl<'a> CrossplatformModuleCollector<'a> {
    fn new(module_prefixes: &'a [Box<str>]) -> Self {
        Self {
            module_prefixes,
            modules: Vec::new(),
            seen: FxHashSet::default(),
        }
    }

    fn collect(mut self, program: &Program<'_>) -> Vec<String> {
        for statement in &program.body {
            match statement {
                Statement::ImportDeclaration(import) => {
                    if let Some(specifiers) = import.specifiers.as_deref() {
                        self.collect_import(
                            (import.source.value.as_str()).unwrap_or_default(),
                            specifiers,
                        );
                    }
                }
                Statement::LazyImportDeclaration(import) => {
                    if let Some(specifiers) = import.specifiers.as_deref() {
                        self.collect_import(
                            (import.source.value.as_str()).unwrap_or_default(),
                            specifiers,
                        );
                    }
                }
                Statement::TSImportEqualsDeclaration(import) => {
                    if let TSModuleReference::ExternalModuleReference(reference) =
                        &import.module_reference
                    {
                        self.collect_import(
                            (reference.expression.value.as_str()).unwrap_or_default(),
                            &[],
                        );
                    }
                }
                _ => {}
            }
        }
        self.modules
    }

    fn collect_import(&mut self, specifier: &str, bindings: &[ImportDeclarationSpecifier<'_>]) {
        let Some(normalized) = self.normalized_system_module(specifier) else {
            return;
        };
        if specifier.starts_with("@kit.") {
            let resolved = bindings
                .iter()
                .filter_map(|binding| match binding {
                    ImportDeclarationSpecifier::ImportSpecifier(binding) => {
                        let name = match &binding.imported {
                            oxc_ast::ast::ModuleExportName::IdentifierName(name) => {
                                name.name.as_str()
                            }
                            oxc_ast::ast::ModuleExportName::IdentifierReference(name) => {
                                name.name.as_str()
                            }
                            oxc_ast::ast::ModuleExportName::StringLiteral(name) => {
                                name.value.as_str().unwrap_or_default()
                            }
                        };
                        resolve_kit_import(specifier, name).map(|entry| entry.module())
                    }
                    _ => None,
                })
                .map(|module| module.trim_start_matches('@').to_string())
                .collect::<Vec<_>>();
            if !resolved.is_empty() {
                for module in resolved {
                    self.push(module);
                }
                return;
            }
        }
        self.push(normalized);
    }

    fn normalized_system_module(&self, specifier: &str) -> Option<String> {
        let request = specifier.strip_prefix('@')?;
        let (prefix, module) = request.split_once('.')?;
        self.module_prefixes
            .iter()
            .any(|candidate| candidate.as_ref() == prefix)
            .then(|| format!("{prefix}.{module}"))
    }

    fn push(&mut self, module: String) {
        if self.seen.insert(module.clone()) {
            self.modules.push(module);
        }
    }
}

struct CrossplatformComponentCollector<'a, 'out> {
    metadata: &'a ComponentMetadata,
    components: &'out mut Vec<String>,
    seen: FxHashSet<String>,
}

impl<'a, 'out> CrossplatformComponentCollector<'a, 'out> {
    fn new(metadata: &'a ComponentMetadata, components: &'out mut Vec<String>) -> Self {
        Self {
            metadata,
            components,
            seen: FxHashSet::default(),
        }
    }

    fn collect_name(&mut self, name: &str) {
        if (self.metadata.is_builtin_component_name(name) || Self::is_js_bind_component(name))
            && self.seen.insert(name.to_string())
        {
            self.components.push(name.to_string());
        }
    }

    fn is_js_bind_component(name: &str) -> bool {
        matches!(
            name,
            "ForEach"
                | "LazyForEach"
                | "TapGesture"
                | "LongPressGesture"
                | "PanGesture"
                | "PinchGesture"
                | "RotationGesture"
                | "GestureGroup"
                | "SwipeGesture"
                | "Gesture"
                | "PanGestureOption"
                | "CustomDialogController"
                | "Storage"
                | "Scroller"
                | "SwiperController"
                | "TabsController"
                | "CalendarController"
                | "AbilityController"
                | "VideoController"
                | "WebController"
                | "XComponentController"
                | "CanvasRenderingContext2D"
                | "CanvasGradient"
                | "ImageBitmap"
                | "ImageData"
                | "Path2D"
                | "RenderingContextSettings"
                | "OffscreenCanvasRenderingContext2D"
                | "DatePickerDialog"
                | "TextPickerDialog"
                | "AlertDialog"
                | "ContextMenu"
                | "ActionSheet"
                | "PatternLockController"
                | "TimePickerDialog"
                | "CalendarPickerDialog"
                | "ArcSwiperController"
        )
    }
}

impl<'program> Visit<'program> for CrossplatformComponentCollector<'_, '_> {
    fn enter_node(&mut self, kind: AstKind<'program>) {
        match kind {
            AstKind::IdentifierName(identifier) => self.collect_name(identifier.name.as_str()),
            AstKind::IdentifierReference(identifier) => self.collect_name(identifier.name.as_str()),
            AstKind::BindingIdentifier(identifier) => self.collect_name(identifier.name.as_str()),
            AstKind::IfStatement(_) => {
                let component_if = self.metadata.runtime_names().component_if.clone();
                if self.seen.insert(component_if.clone()) {
                    self.components.push(component_if);
                }
            }
            _ => {}
        }
    }
}
