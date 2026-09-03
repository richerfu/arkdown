use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::Visit;
use oxc_semantic::Scoping;
use oxc_span::Span;
use oxc_syntax::symbol::SymbolId;
use rustc_hash::FxHashSet;

use crate::error::{DiagnosticSeverity, EtsTransformError};

use super::api_available::ApiAvailableTransformOptions;

/// Validates source-retention `@Available` annotations resolved from the SDK.
///
/// This mirrors
/// `fast_build/system_api/api_check_utils.ts::isSourceRetentionAnnotationContentValid`.
/// The TypeScript implementation first resolves `annotationDeclaration` to
/// `@ohos.annotation.d.ets`; this native pass preserves that identity contract
/// by recording only `Available` bindings imported from the owning SDK module
/// or its `BasicServicesKit` re-export.
pub(crate) struct AvailableAnnotationValidator<'a> {
    source_id: &'a str,
    options: ApiAvailableTransformOptions,
}

impl<'a> AvailableAnnotationValidator<'a> {
    pub(crate) fn new(source_id: &'a str, options: ApiAvailableTransformOptions) -> Self {
        Self { source_id, options }
    }

    pub(crate) fn diagnostics_with_scoping<'program>(
        &self,
        program: &'program Program<'program>,
        scoping: &Scoping,
    ) -> Vec<EtsTransformError> {
        let bindings = AvailableSdkBindings::from_program(program);
        if bindings.is_empty() {
            return Vec::new();
        }
        let mut visitor = AvailableAnnotationVisitor {
            source_id: self.source_id,
            runtime_os: self.options.runtime_os(),
            bindings,
            scoping,
            parent_versions: Vec::new(),
            diagnostics: Vec::new(),
        };
        visitor.visit_program(program);
        visitor.diagnostics
    }
}

#[derive(Default)]
struct AvailableSdkBindings {
    symbols: FxHashSet<SymbolId>,
}

impl AvailableSdkBindings {
    fn from_program(program: &Program<'_>) -> Self {
        let mut bindings = Self::default();
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if !matches!(
                import.source.value.as_str(),
                "@ohos.annotation" | "@ohos:annotation" | "@kit.BasicServicesKit"
            ) {
                continue;
            }
            let Some(specifiers) = import.specifiers.as_ref() else {
                continue;
            };
            for specifier in specifiers {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                if ModuleExportNameText::new(&specifier.imported).as_str() == "Available" {
                    bindings.symbols.insert(specifier.local.symbol_id());
                }
            }
        }
        bindings
    }

    fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    fn contains(&self, reference: &IdentifierReference<'_>, scoping: &Scoping) -> bool {
        reference
            .reference_id
            .get()
            .and_then(|reference_id| scoping.get_reference(reference_id).symbol_id())
            .is_some_and(|symbol_id| self.symbols.contains(&symbol_id))
    }
}

struct ModuleExportNameText<'a> {
    name: &'a ModuleExportName<'a>,
}

impl<'a> ModuleExportNameText<'a> {
    fn new(name: &'a ModuleExportName<'a>) -> Self {
        Self { name }
    }

    fn as_str(&self) -> &str {
        match self.name {
            ModuleExportName::IdentifierName(identifier) => identifier.name.as_str(),
            ModuleExportName::IdentifierReference(identifier) => identifier.name.as_str(),
            ModuleExportName::StringLiteral(literal) => literal.value.as_str(),
        }
    }
}

#[derive(Clone, Copy)]
struct AvailableVersion<'a> {
    os: &'a str,
    version: &'a str,
}

impl<'a> AvailableVersion<'a> {
    fn parse(raw: &'a str) -> Self {
        let mut parts = raw.split_whitespace();
        let first = parts.next().unwrap_or_default();
        match parts.next() {
            Some(version) => Self { os: first, version },
            None => Self {
                os: "OpenHarmony",
                version: first,
            },
        }
    }

    fn format_is_valid(self) -> bool {
        OpenHarmonyVersion::new(self.version).is_valid()
    }

    fn is_at_least(self, required: Self) -> bool {
        PointVersion::new(self.version) >= PointVersion::new(required.version)
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct PointVersion([u32; 3]);

impl PointVersion {
    fn new(version: &str) -> Self {
        let mut values = [0; 3];
        for (index, segment) in version.split('.').take(3).enumerate() {
            values[index] = segment.parse().unwrap_or(0);
        }
        Self(values)
    }
}

struct OpenHarmonyVersion<'a> {
    value: &'a str,
}

impl<'a> OpenHarmonyVersion<'a> {
    fn new(value: &'a str) -> Self {
        Self { value }
    }

    fn is_valid(&self) -> bool {
        let segments = self.value.split('.').collect::<Vec<_>>();
        match segments.as_slice() {
            [major] => Self::decimal_in_range(major, 1, 999),
            [major, minor, patch] => {
                Self::decimal_in_range(major, 26, 99)
                    && Self::decimal_segment_in_range(minor, 0, 99)
                    && Self::decimal_segment_in_range(patch, 0, 99)
            }
            _ => false,
        }
    }

    fn decimal_in_range(value: &str, minimum: u32, maximum: u32) -> bool {
        !value.is_empty()
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && (value == "0" || !value.starts_with('0'))
            && value
                .parse::<u32>()
                .is_ok_and(|value| (minimum..=maximum).contains(&value))
    }

    fn decimal_segment_in_range(value: &str, minimum: u32, maximum: u32) -> bool {
        !value.is_empty()
            && value.len() <= 2
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && value
                .parse::<u32>()
                .is_ok_and(|value| (minimum..=maximum).contains(&value))
    }
}

struct AvailableAnnotationVisitor<'ast, 'context> {
    source_id: &'context str,
    runtime_os: &'static str,
    bindings: AvailableSdkBindings,
    scoping: &'context Scoping,
    parent_versions: Vec<Option<AvailableVersion<'ast>>>,
    diagnostics: Vec<EtsTransformError>,
}

impl<'ast> AvailableAnnotationVisitor<'ast, '_> {
    const FORMAT_DETAIL: &'static str = "The OpenHarmony version must be an integer between 1 and 999,\n and when the OpenHarmony version is greater than or equal to 26, the version number format also supports the M.S.F format.";

    fn enter_decorated_node(&mut self, decorators: &'ast [Decorator<'ast>]) {
        let mut current_version = None;
        for decorator in decorators {
            let Some(version) = self.available_version(decorator) else {
                continue;
            };
            let valid = self.validate_version(version, decorator.span);
            if valid {
                if let Some(parent) = self.parent_versions.iter().rev().flatten().next() {
                    if version.os == parent.os && !version.is_at_least(*parent) {
                        self.diagnostics.push(EtsTransformError::Diagnostic {
                            file: self.source_id.to_string(),
                            message: format!(
                                "Unnecessary. The outer annotation already indicates that the version is greater than or equal to {}.",
                                parent.version
                            ),
                            severity: DiagnosticSeverity::Warning,
                        });
                    }
                }
                current_version.get_or_insert(version);
            }
        }
        self.parent_versions.push(current_version);
    }

    fn leave_decorated_node(&mut self) {
        self.parent_versions.pop();
    }

    fn available_version(
        &self,
        decorator: &'ast Decorator<'ast>,
    ) -> Option<AvailableVersion<'ast>> {
        let Expression::CallExpression(call) = &decorator.expression else {
            return None;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };
        if !self.bindings.contains(callee, self.scoping) {
            return None;
        }
        let Expression::ObjectExpression(options) = call.arguments.first()?.as_expression()? else {
            return None;
        };
        for property in &options.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            if PropertyName::new(&property.key).as_str() != Some("minApiVersion") {
                continue;
            }
            let Expression::StringLiteral(value) = &property.value else {
                continue;
            };
            return Some(AvailableVersion::parse(value.value.as_str()));
        }
        None
    }

    fn validate_version(&mut self, version: AvailableVersion<'_>, span: Span) -> bool {
        if version.os != "OpenHarmony" && version.os != self.runtime_os {
            self.diagnostics.push(EtsTransformError::CodedDiagnostic {
                file: self.source_id.to_string(),
                code: "11706017".to_string(),
                message: format!(
                    "The runtime OS for the current project is {}. @Available is not supported on the OS: {}.",
                    self.runtime_os, version.os
                ),
                severity: DiagnosticSeverity::Error,
                span: Some(span),
                solutions: vec![
                    "Use the correct OS name matching the project runtime OS.".to_string(),
                ],
            });
            return false;
        }
        if version.format_is_valid() {
            return true;
        }
        self.diagnostics.push(EtsTransformError::CodedDiagnostic {
            file: self.source_id.to_string(),
            code: "11706016".to_string(),
            message: format!(
                "The runtime OS for the current project is {}. The OS version number {} is invalid. {}",
                self.runtime_os,
                version.version,
                Self::FORMAT_DETAIL
            ),
            severity: DiagnosticSeverity::Error,
            span: Some(span),
            solutions: vec![
                "Change the version number to an integer between 1 and 999, or use the standardized M.S.F format."
                    .to_string(),
            ],
        });
        false
    }

    fn decorators(kind: AstKind<'ast>) -> Option<&'ast [Decorator<'ast>]> {
        match kind {
            AstKind::VariableDeclaration(node) => {
                node.decorators.as_deref().map(|value| value.as_slice())
            }
            AstKind::Function(node) => Some(node.decorators.as_slice()),
            AstKind::FormalParameter(node) => Some(node.decorators.as_slice()),
            AstKind::FormalParameterRest(node) => Some(node.decorators.as_slice()),
            AstKind::Class(node) => Some(node.decorators.as_slice()),
            AstKind::MethodDefinition(node) => Some(node.decorators.as_slice()),
            AstKind::PropertyDefinition(node) => Some(node.decorators.as_slice()),
            AstKind::AccessorProperty(node) => Some(node.decorators.as_slice()),
            AstKind::ExportDeclaration(node) => Some(node.decorators.as_slice()),
            AstKind::TSEnumDeclaration(node) => {
                node.decorators.as_deref().map(|value| value.as_slice())
            }
            AstKind::TSTypeAliasDeclaration(node) => {
                node.decorators.as_deref().map(|value| value.as_slice())
            }
            AstKind::TSInterfaceDeclaration(node) => {
                node.decorators.as_deref().map(|value| value.as_slice())
            }
            AstKind::StructStatement(node) => Some(node.decorators.as_slice()),
            _ => None,
        }
    }
}

impl<'ast> Visit<'ast> for AvailableAnnotationVisitor<'ast, '_> {
    fn enter_node(&mut self, kind: AstKind<'ast>) {
        if let Some(decorators) = Self::decorators(kind) {
            self.enter_decorated_node(decorators);
        }
    }

    fn leave_node(&mut self, kind: AstKind<'ast>) {
        if Self::decorators(kind).is_some() {
            self.leave_decorated_node();
        }
    }
}

struct PropertyName<'a> {
    key: &'a PropertyKey<'a>,
}

impl<'a> PropertyName<'a> {
    fn new(key: &'a PropertyKey<'a>) -> Self {
        Self { key }
    }

    fn as_str(&self) -> Option<&str> {
        match self.key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str()),
            PropertyKey::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        }
    }
}
