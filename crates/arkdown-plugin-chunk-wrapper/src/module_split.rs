use oxc_allocator::Allocator;
use oxc_ast::ast::{ExportDefaultDeclarationKind, Program, Statement};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::source_edit::{apply_source_patches, SourcePatch};

pub(crate) fn split_imports(source: &str) -> (String, String) {
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::tsx()).parse();

    if parse_result.panicked {
        return (String::new(), source.to_owned());
    }

    ModuleImportSplitter::new(source, &parse_result.program).split()
}

struct ModuleImportSplitter<'a> {
    source: &'a str,
    imports: String,
    patches: Vec<SourcePatch>,
}

impl<'a> ModuleImportSplitter<'a> {
    fn new(source: &'a str, program: &Program<'_>) -> Self {
        let mut splitter = Self {
            source,
            imports: String::new(),
            patches: Vec::new(),
        };
        splitter.collect(program);
        splitter
    }

    fn split(self) -> (String, String) {
        if self.patches.is_empty() {
            return (String::new(), self.source.to_owned());
        }

        let body = apply_source_patches(self.source, self.patches);
        (self.imports, body)
    }

    fn collect(&mut self, program: &Program<'_>) {
        for statement in &program.body {
            match statement {
                Statement::ImportDeclaration(import) => {
                    self.imports
                        .push_str(self.span_text(import.span.start, import.span.end));
                    self.imports.push('\n');
                    self.patches.push(SourcePatch {
                        start: import.span.start as usize,
                        end: import.span.end as usize,
                        replacement: preserve_newlines(
                            self.span_text(import.span.start, import.span.end),
                        ),
                    });
                }
                Statement::ExportNamedDeclaration(export) => {
                    let replacement =
                        export
                            .declaration
                            .as_ref()
                            .map_or_else(String::new, |declaration| {
                                self.span_text(declaration.span().start, export.span.end)
                                    .to_owned()
                            });
                    self.patches.push(SourcePatch {
                        start: export.span.start as usize,
                        end: export.span.end as usize,
                        replacement,
                    });
                }
                Statement::ExportDefaultDeclaration(export) => {
                    self.patches.push(SourcePatch {
                        start: export.span.start as usize,
                        end: export.span.end as usize,
                        replacement: self.default_export_declaration_text(&export.declaration),
                    });
                }
                Statement::ExportAllDeclaration(export) => {
                    self.patches.push(SourcePatch {
                        start: export.span.start as usize,
                        end: export.span.end as usize,
                        replacement: String::new(),
                    });
                }
                _ => {}
            }
        }
    }

    fn default_export_declaration_text(
        &self,
        declaration: &ExportDefaultDeclarationKind<'_>,
    ) -> String {
        self.span_text(declaration.span().start, declaration.span().end)
            .to_owned()
    }

    fn span_text(&self, start: u32, end: u32) -> &'a str {
        &self.source[start as usize..end as usize]
    }
}

fn preserve_newlines(text: &str) -> String {
    let count = text
        .as_bytes()
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    "\n".repeat(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_imports_basic() {
        let code = r#"import { UIAbility } from "@kit.AbilityKit";
const x = 1;"#;
        let (imports, body) = split_imports(code);
        assert!(imports.contains(r#"import { UIAbility } from "@kit.AbilityKit""#));
        assert!(!body.contains("import"));
        assert!(body.contains("const x = 1;"));
    }

    #[test]
    fn strips_export_default() {
        let code = r#"export default class Foo {}"#;
        let (imports, body) = split_imports(code);
        assert!(imports.is_empty());
        assert!(!body.contains("export"));
        assert!(body.contains("class Foo {}"));
    }

    #[test]
    fn strips_named_export_declaration() {
        let code = r#"export const value = 1;"#;
        let (imports, body) = split_imports(code);
        assert!(imports.is_empty());
        assert_eq!(body, "const value = 1;");
    }

    #[test]
    fn removes_re_export_list() {
        let code = r#"export { value };"#;
        let (imports, body) = split_imports(code);
        assert!(imports.is_empty());
        assert_eq!(body, "");
    }

    #[test]
    fn split_imports_no_imports() {
        let code = "const x = 1;\n";
        let (imports, body) = split_imports(code);
        assert!(imports.is_empty());
        assert_eq!(body, code);
    }
}
