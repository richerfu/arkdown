//! OXC-owned insertion of native Visual/SuperVisual ArkTS fragments.
//!
//! `.visual` decoding and fragment generation are implemented in Rust. This
//! pass locates the ArkTS struct and inserts those fragments using OXC spans.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, MethodDefinition, Program, Statement, StructElement,
    StructStatement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use serde::{Deserialize, Serialize};

use crate::error::EtsTransformError;
use crate::source_map::{offset_to_line_col, EtsSourceMapBuilder, MapEntry};

/// ArkTS fragments generated from a Visual document.
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

#[derive(Debug)]
pub(crate) struct VisualSourceTransform {
    pub(crate) code: String,
    pub(crate) source_map: String,
}

impl<'a> VisualSourceInjector<'a> {
    pub(crate) fn new(source: &'a str, id: &'a str, generated: &'a VisualGeneratedSource) -> Self {
        Self {
            source,
            id,
            generated,
        }
    }

    pub(crate) fn transform(&self) -> Result<Option<VisualSourceTransform>, EtsTransformError> {
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
    original_line: u32,
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
            self.push_at_line(0, 0, format!("{}\n", self.generated.ets_import));
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
        let original_line = offset_to_line_col(self.source, offset as u32).0 + 1;
        self.push_at_line(offset, original_line, content);
    }

    fn push_at_line(&mut self, offset: usize, original_line: u32, content: String) {
        self.insertions.push(VisualInsertion {
            offset,
            original_line,
            sequence: self.sequence,
            content,
        });
        self.sequence += 1;
    }

    fn apply(mut self) -> Result<Option<VisualSourceTransform>, EtsTransformError> {
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
        let shifts = self
            .insertions
            .iter()
            .map(|insertion| {
                (
                    insertion.original_line,
                    insertion
                        .content
                        .bytes()
                        .filter(|byte| *byte == b'\n')
                        .count() as u32,
                )
            })
            .collect::<Vec<_>>();
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
        let entries = self
            .source
            .split('\n')
            .enumerate()
            .map(|(source_line, _)| {
                let original_line = source_line as u32 + 1;
                let inserted_lines = shifts
                    .iter()
                    .filter(|(insertion_line, _)| original_line > *insertion_line)
                    .map(|(_, lines)| *lines)
                    .sum::<u32>();
                MapEntry {
                    generated_line: (original_line + inserted_lines - 1) as usize,
                    generated_col: 0,
                    src_line: source_line as u32,
                    src_col: 0,
                }
            })
            .collect::<Vec<_>>();
        let source_map =
            EtsSourceMapBuilder::new(self.id, output.lines().count(), &entries).build();
        Ok(Some(VisualSourceTransform {
            code: output,
            source_map,
        }))
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

        assert!(output.code.starts_with("import mediaquery"));
        assert!(output.code.contains("private listener"));
        assert!(output.code.contains("aboutToAppear()"));
        assert!(output.code.contains("build() {\n    Column()\n}"));
        assert!(
            output.code.find("private listener").unwrap()
                < output.code.find("aboutToAppear").unwrap()
        );
        assert!(output.code.find("aboutToAppear").unwrap() < output.code.find("build()").unwrap());
        let source_map: serde_json::Value =
            serde_json::from_str(&output.source_map).expect("Visual source map");
        assert_eq!(source_map["sources"][0], "Index.ets");
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
