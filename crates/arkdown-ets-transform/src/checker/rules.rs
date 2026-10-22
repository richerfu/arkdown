use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, ImportDeclarationSpecifier, Program, Statement,
    StructStatement,
};

use crate::error::{DiagnosticSeverity, EtsTransformError};
use crate::transform::component_meta::ComponentMetadata;
use crate::transform::decorator::{ArkuiDecorator, ArkuiDecoratorName};
use crate::transform::{
    ArktsFileTypeFacts, ClassObservation, CustomComponentUpdateProps, ImportedClassObservations,
};
use crate::ModuleExportNameText;
use rustc_hash::FxHashMap;

use super::arkts11_class::Arkts11ClassValidator;
use super::arkts11_function::Arkts11FunctionValidator;
use super::arkts11_struct::Arkts11StructValidator;

/// Deterministic diagnostics emitted by one OXC ArkTS 1.1 source check.
///
/// The report owns diagnostics rather than collapsing them into a transform
/// error. The coordinator can therefore consume one ordered native report
/// after the remaining project-semantic rules have moved to Rust.
#[derive(Debug, Default)]
pub struct EtsCheckReport {
    diagnostics: Vec<EtsTransformError>,
}

impl EtsCheckReport {
    pub fn diagnostics(&self) -> &[EtsTransformError] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&EtsTransformError> {
        self.diagnostics
            .iter()
            .find(|diagnostic| diagnostic.is_error())
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_error())
            .count()
    }

    pub fn into_diagnostics(self) -> Vec<EtsTransformError> {
        self.diagnostics
    }

    pub(crate) fn extend(&mut self, diagnostics: impl IntoIterator<Item = EtsTransformError>) {
        self.diagnostics.extend(diagnostics);
    }
}

/// ArkTS 1.1 source checker operating on the same OXC program consumed by the
/// transform pipeline. It performs no filesystem or SDK reads.
pub struct EtsChecker<'program, 'alloc, 'file> {
    program: &'program Program<'alloc>,
    file: &'file str,
    page_context: Option<PageValidationContext>,
    component_metadata: Option<&'program ComponentMetadata>,
    imported_class_observations: ImportedClassObservations,
    custom_component_info: Option<&'program CustomComponentUpdateProps>,
    type_facts: Option<&'program ArktsFileTypeFacts>,
}

impl<'program, 'alloc, 'file> EtsChecker<'program, 'alloc, 'file> {
    pub fn new(program: &'program Program<'alloc>, file: &'file str) -> Self {
        Self {
            program,
            file,
            page_context: None,
            component_metadata: None,
            imported_class_observations: ImportedClassObservations::default(),
            custom_component_info: None,
            type_facts: None,
        }
    }

    pub fn with_component_metadata(
        mut self,
        component_metadata: &'program ComponentMetadata,
    ) -> Self {
        self.component_metadata = Some(component_metadata);
        self
    }

    pub fn with_page_context(
        mut self,
        entry_source: bool,
        preview_mode: bool,
        check_entry: bool,
    ) -> Self {
        self.page_context = Some(PageValidationContext {
            entry_source,
            preview_mode,
            check_entry,
        });
        self
    }

    pub(crate) fn with_imported_class_observations(
        mut self,
        observations: ImportedClassObservations,
    ) -> Self {
        self.imported_class_observations = observations;
        self
    }

    pub(crate) fn with_custom_component_info(
        mut self,
        custom_component_info: &'program CustomComponentUpdateProps,
    ) -> Self {
        self.custom_component_info = Some(custom_component_info);
        self
    }

    pub(crate) fn with_type_facts(
        mut self,
        type_facts: Option<&'program ArktsFileTypeFacts>,
    ) -> Self {
        self.type_facts = type_facts;
        self
    }

    pub fn check(&self) -> EtsCheckReport {
        let mut report = EtsCheckReport::default();
        let mut page_decorators = PageDecoratorCounts::default();
        let component_reuse = self.component_reuse_inventory();
        let class_observations = self.class_observation_inventory();
        let sdk_type_bindings = self.sdk_type_bindings();

        for statement in &self.program.body {
            match statement {
                Statement::StructStatement(structure) => {
                    self.check_struct(
                        structure,
                        false,
                        &component_reuse,
                        &class_observations,
                        &mut report,
                    );
                    page_decorators.collect(structure);
                }
                Statement::ClassDeclaration(class) => {
                    Arkts11ClassValidator::new(class, self.file, &mut report.diagnostics)
                        .with_class_observation_inventory(&class_observations)
                        .with_sdk_type_bindings(&sdk_type_bindings)
                        .check();
                }
                Statement::FunctionDeclaration(function) => {
                    Arkts11FunctionValidator::new(function, self.file, &mut report.diagnostics)
                        .with_component_metadata(self.component_metadata)
                        .check();
                }
                Statement::ExportDeclaration(export) => match &export.declaration {
                    Declaration::StructStatement(structure) => {
                        self.check_struct(
                            structure,
                            true,
                            &component_reuse,
                            &class_observations,
                            &mut report,
                        );
                        page_decorators.collect(structure);
                    }
                    Declaration::ClassDeclaration(class) => {
                        Arkts11ClassValidator::new(class, self.file, &mut report.diagnostics)
                            .with_class_observation_inventory(&class_observations)
                            .with_sdk_type_bindings(&sdk_type_bindings)
                            .check();
                    }
                    Declaration::FunctionDeclaration(function) => {
                        Arkts11FunctionValidator::new(function, self.file, &mut report.diagnostics)
                            .with_component_metadata(self.component_metadata)
                            .check_with_extra_decorators(&export.decorators);
                    }
                    _ => {}
                },
                Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                    ExportDefaultDeclarationKind::StructStatement(structure) => {
                        self.check_struct(
                            structure,
                            true,
                            &component_reuse,
                            &class_observations,
                            &mut report,
                        );
                        page_decorators.collect(structure);
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        Arkts11ClassValidator::new(class, self.file, &mut report.diagnostics)
                            .with_class_observation_inventory(&class_observations)
                            .with_sdk_type_bindings(&sdk_type_bindings)
                            .check();
                    }
                    ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                        Arkts11FunctionValidator::new(function, self.file, &mut report.diagnostics)
                            .with_component_metadata(self.component_metadata)
                            .check();
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        if let Some(context) = self.page_context {
            page_decorators.validate(context, self.file, &mut report.diagnostics);
        }
        report
    }

    fn sdk_type_bindings(&self) -> rustc_hash::FxHashSet<String> {
        let mut bindings = rustc_hash::FxHashSet::default();
        for statement in &self.program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if !matches!(
                import.source.value.as_str(),
                Some("@ohos.arkui.StateManagement")
                    | Some("@ohos:arkui.StateManagement")
                    | Some("@kit.ArkUI")
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
                if ModuleExportNameText::new(&specifier.imported).binding_name()
                    == ArkuiDecoratorName::TYPE
                {
                    bindings.insert(specifier.local.name.to_string());
                }
            }
        }
        bindings
    }

    fn check_struct(
        &self,
        statement: &StructStatement<'_>,
        exported: bool,
        component_reuse: &FxHashMap<String, bool>,
        class_observations: &ImportedClassObservations,
        report: &mut EtsCheckReport,
    ) {
        Arkts11StructValidator::new(statement, self.file, &mut report.diagnostics)
            .with_exported(exported)
            .with_component_metadata(self.component_metadata)
            .with_component_reuse_inventory(component_reuse)
            .with_class_observation_inventory(class_observations)
            .with_custom_component_info(self.custom_component_info)
            .with_type_facts(self.type_facts)
            .check();
    }

    fn component_reuse_inventory(&self) -> FxHashMap<String, bool> {
        let mut inventory = FxHashMap::default();
        for statement in &self.program.body {
            let structure = match statement {
                Statement::StructStatement(structure) => Some(structure.as_ref()),
                Statement::ExportDeclaration(export) => match &export.declaration {
                    Declaration::StructStatement(structure) => Some(structure.as_ref()),
                    _ => None,
                },
                Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                    ExportDefaultDeclarationKind::StructStatement(structure) => {
                        Some(structure.as_ref())
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(structure) = structure else {
                continue;
            };
            let reusable = structure.decorators.iter().any(|decorator| {
                matches!(
                    ArkuiDecorator::new(decorator).name(),
                    ArkuiDecoratorName::REUSABLE | ArkuiDecoratorName::REUSABLE_V2
                )
            });
            inventory.insert(structure.id.name.to_string(), reusable);
        }
        inventory
    }

    fn class_observation_inventory(&self) -> ImportedClassObservations {
        let mut inventory = self.imported_class_observations.clone();
        for statement in &self.program.body {
            let class = match statement {
                Statement::ClassDeclaration(class) => Some(class.as_ref()),
                Statement::ExportDeclaration(export) => match &export.declaration {
                    Declaration::ClassDeclaration(class) => Some(class.as_ref()),
                    _ => None,
                },
                Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => Some(class.as_ref()),
                    _ => None,
                },
                _ => None,
            };
            let Some(class) = class else {
                continue;
            };
            let Some(name) = class.id.as_ref() else {
                continue;
            };
            let observed_v1 = class.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::OBSERVED
            });
            let observed_v2 = class.decorators.iter().any(|decorator| {
                ArkuiDecorator::new(decorator).name() == ArkuiDecoratorName::OBSERVED_V2
            });
            inventory.bindings.insert(
                name.name.to_string(),
                ClassObservation {
                    observed_v1,
                    observed_v2,
                },
            );
        }
        inventory
    }
}

#[derive(Clone, Copy)]
struct PageValidationContext {
    entry_source: bool,
    preview_mode: bool,
    check_entry: bool,
}

#[derive(Default)]
struct PageDecoratorCounts {
    entry: usize,
    preview: usize,
}

impl PageDecoratorCounts {
    fn collect(&mut self, statement: &StructStatement<'_>) {
        for decorator in &statement.decorators {
            match ArkuiDecorator::new(decorator).name() {
                ArkuiDecoratorName::ENTRY => self.entry += 1,
                ArkuiDecoratorName::PREVIEW => self.preview += 1,
                _ => {}
            }
        }
    }

    fn validate(
        &self,
        context: PageValidationContext,
        file: &str,
        diagnostics: &mut Vec<EtsTransformError>,
    ) {
        if !context.entry_source {
            return;
        }
        if self.preview > 10 {
            self.push_error(
                file,
                "10905404",
                "A page can contain at most 10 '@Preview' decorators.",
                &[],
                diagnostics,
            );
        }
        if self.entry > 1 {
            self.push_error(
                file,
                "10905231",
                "A page can't contain more than one '@Entry' decorator.",
                &[],
                diagnostics,
            );
        }
        if context.preview_mode && !context.check_entry {
            if self.preview == 0 && self.entry != 1 {
                self.push_error(
                    file,
                    "10905403",
                    "A page which is being previewed must have one and only one '@Entry' decorator, or at least one '@Preview' decorator.",
                    &[],
                    diagnostics,
                );
            }
        } else if self.entry != 1 {
            self.push_error(
                file,
                "10905402",
                "A page configured in 'main_pages.json or build-profile.json5' must have one and only one '@Entry' decorator.",
                &["Please make sure that the splash page has one and only one '@Entry' decorator."],
                diagnostics,
            );
        }
    }

    fn push_error(
        &self,
        file: &str,
        code: &str,
        message: &str,
        solutions: &[&str],
        diagnostics: &mut Vec<EtsTransformError>,
    ) {
        diagnostics.push(EtsTransformError::CodedDiagnostic {
            file: file.to_string(),
            code: code.to_string(),
            message: message.to_string(),
            severity: DiagnosticSeverity::Error,
            span: None,
            solutions: solutions
                .iter()
                .map(|solution| (*solution).to_string())
                .collect(),
        });
    }
}
