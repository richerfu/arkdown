//! OXC-owned insertion of SDK-generated Visual/SuperVisual ArkTS fragments.
//!
//! The SDK's `codegen_ets.js::genETS` remains a JavaScript compatibility
//! boundary because the generator is shipped as a JavaScript SDK artifact.
//! Discovery, validation, insertion ordering, and compilation remain in Rust.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, MethodDefinition, Program, Statement, StructElement,
    StructStatement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use serde::{Deserialize, Serialize};

use crate::error::EtsTransformError;

/// ArkTS fragments returned by the SDK `genETS` compatibility module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualGeneratedSource {
    #[serde(default)]
    pub build: String,
    #[serde(default)]
    pub ets_import: String,
    #[serde(default)]
    pub ets_variable: String,
    #[serde(default)]
    pub ets_function: String,
    #[serde(default)]
    pub about_to_appear: String,
}

pub(crate) struct VisualSourceInjector<'a> {
    source: &'a str,
    id: &'a str,
    generated: &'a VisualGeneratedSource,
}

impl<'a> VisualSourceInjector<'a> {
    pub(crate) fn new(source: &'a str, id: &'a str, generated: &'a VisualGeneratedSource) -> Self {
        Self {
            source,
            id,
            generated,
        }
    }

    pub(crate) fn transform(&self) -> Result<Option<String>, EtsTransformError> {
        if self.generated == &VisualGeneratedSource::default() {
            return Ok(None);
        }
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, self.source, SourceType::ets()).parse();
        if parsed.panicked || !parsed.diagnostics.is_empty() {
            return Ok(None);
        }

        let mut plan = VisualInsertionPlan::new(self.source, self.id, self.generated);
        plan.collect_program(&parsed.program)?;
        plan.apply()
    }
}

#[derive(Debug)]
struct VisualInsertion {
    offset: usize,
    sequence: usize,
    content: String,
}

struct VisualInsertionPlan<'a> {
    source: &'a str,
    id: &'a str,
    generated: &'a VisualGeneratedSource,
    insertions: Vec<VisualInsertion>,
    sequence: usize,
    saw_struct: bool,
}

impl<'a> VisualInsertionPlan<'a> {
    fn new(source: &'a str, id: &'a str, generated: &'a VisualGeneratedSource) -> Self {
        Self {
            source,
            id,
            generated,
            insertions: Vec::new(),
            sequence: 0,
            saw_struct: false,
        }
    }

    fn collect_program(&mut self, program: &Program<'_>) -> Result<(), EtsTransformError> {
        for statement in &program.body {
            match statement {
                Statement::StructStatement(statement) => self.collect_struct(statement)?,
                Statement::ExportDeclaration(export) => {
                    if let Declaration::StructStatement(statement) = &export.declaration {
                        self.collect_struct(statement)?;
                    }
                }
                Statement::ExportDefaultDeclaration(export) => {
                    if let ExportDefaultDeclarationKind::StructStatement(statement) =
                        &export.declaration
                    {
                        self.collect_struct(statement)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn collect_struct(&mut self, statement: &StructStatement<'_>) -> Result<(), EtsTransformError> {
        self.saw_struct = true;
        let build = statement.body.body.iter().find_map(|element| {
            let StructElement::MethodDefinition(method) = element else {
                return None;
            };
            (method.key.static_name().as_deref() == Some("build")).then_some(method)
        });
        let Some(build) = build else {
            return Ok(());
        };
        self.validate_empty_build(build)?;

        if !self.generated.ets_import.is_empty() {
            self.push(0, format!("{}\n", self.generated.ets_import));
        }
        let fields = format!(
            "{}{}",
            self.generated.ets_variable, self.generated.ets_function
        );
        if !fields.is_empty() {
            self.push(build.span().start as usize, format!("\n{fields}"));
        }
        if !self.generated.build.is_empty() {
            let body = build
                .value
                .body
                .as_deref()
                .ok_or_else(|| self.invalid_build())?;
            self.push(
                body.span.start as usize + 1,
                format!("\n{}\n", self.generated.build),
            );
        }
        if !self.generated.about_to_appear.is_empty() {
            if let Some(method) = Self::method(statement, "aboutToAppear") {
                let body = method
                    .value
                    .body
                    .as_deref()
                    .ok_or_else(|| self.invalid_build())?;
                self.push(
                    body.span.start as usize + 1,
                    format!("\n{}", self.generated.about_to_appear),
                );
            } else {
                self.push(
                    build.span().start as usize,
                    format!(
                        "\n  aboutToAppear() {{\n{}  }}\n",
                        self.generated.about_to_appear
                    ),
                );
            }
        }
        Ok(())
    }

    fn method<'b>(
        statement: &'b StructStatement<'_>,
        name: &str,
    ) -> Option<&'b MethodDefinition<'b>> {
        statement
            .body
            .body
            .iter()
            .find_map(|element| {
                let StructElement::MethodDefinition(method) = element else {
                    return None;
                };
                (method.key.static_name().as_deref() == Some(name)).then_some(method)
            })
            .map(|method| &**method)
    }

    fn validate_empty_build(&self, build: &MethodDefinition<'_>) -> Result<(), EtsTransformError> {
        let span = build.span();
        let text = self
            .source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default();
        let normalized = text.replace(' ', "").replace(['\r', '\n'], "");
        if normalized == "build(){}" {
            return Ok(());
        }
        Err(self.invalid_build())
    }

    fn invalid_build(&self) -> EtsTransformError {
        EtsTransformError::Diagnostic {
            file: self.id.to_string(),
            message: "when the corresponding visual file exists, the build function of the entry component must be empty.".to_string(),
            severity: crate::error::DiagnosticSeverity::Error,
        }
    }

    fn push(&mut self, offset: usize, content: String) {
        self.insertions.push(VisualInsertion {
            offset,
            sequence: self.sequence,
            content,
        });
        self.sequence += 1;
    }

    fn apply(mut self) -> Result<Option<String>, EtsTransformError> {
        if !self.saw_struct || self.insertions.is_empty() {
            return Ok(None);
        }
        self.insertions
            .sort_by_key(|insertion| (insertion.offset, insertion.sequence));
        let mut output = String::with_capacity(
            self.source.len()
                + self
                    .insertions
                    .iter()
                    .map(|insertion| insertion.content.len())
                    .sum::<usize>(),
        );
        let mut cursor = 0;
        for insertion in self.insertions {
            let offset = insertion.offset.min(self.source.len());
            if offset < cursor || !self.source.is_char_boundary(offset) {
                return Err(EtsTransformError::Fatal {
                    file: self.id.to_string(),
                    message: "Visual insertion produced an invalid OXC source span".to_string(),
                });
            }
            output.push_str(&self.source[cursor..offset]);
            output.push_str(&insertion.content);
            cursor = offset;
        }
        output.push_str(&self.source[cursor..]);
        Ok(Some(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_sdk_fragments_in_source_plugin_order() {
        let source = "@Entry\n@Component\nstruct Index {\n  build() {}\n}\n";
        let generated = VisualGeneratedSource {
            ets_import: "import mediaquery from '@ohos.mediaquery';".to_string(),
            ets_variable: "  private listener = mediaquery.matchMediaSync('(width > 0)');\n"
                .to_string(),
            ets_function: "  onChange() {}\n".to_string(),
            build: "    Column()".to_string(),
            about_to_appear: "    this.listener.on('change', this.onChange);\n".to_string(),
        };

        let output = VisualSourceInjector::new(source, "Index.ets", &generated)
            .transform()
            .expect("visual transform")
            .expect("visual output");

        assert!(output.starts_with("import mediaquery"));
        assert!(output.contains("private listener"));
        assert!(output.contains("aboutToAppear()"));
        assert!(output.contains("build() {\n    Column()\n}"));
        assert!(output.find("private listener").unwrap() < output.find("aboutToAppear").unwrap());
        assert!(output.find("aboutToAppear").unwrap() < output.find("build()").unwrap());
    }

    #[test]
    fn rejects_nonempty_build_like_process_visual() {
        let generated = VisualGeneratedSource {
            build: "    Column()".to_string(),
            ..VisualGeneratedSource::default()
        };
        let error = VisualSourceInjector::new(
            "@Component struct Index { build() { Text('source') } }",
            "Index.ets",
            &generated,
        )
        .transform()
        .expect_err("nonempty source build must be rejected");

        assert!(error.to_string().contains("build function"));
    }
}
