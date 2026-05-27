use oxc_allocator::Allocator;
use oxc_ast::ast::{ModuleExportName, Statement};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use arkdown_core::{ExportBinding, ImportEdge, ModuleSource, ModuleType, ResolvedId};

pub struct ParsedModule {
    pub imports: Vec<ImportEdge>,
    pub exports: Vec<ExportBinding>,
    pub has_side_effects: bool,
}

/// Infer [`SourceType`] from our [`ModuleType`] and the file path.
fn source_type_for(source: &ModuleSource) -> SourceType {
    // Prefer path-based inference (handles .ets, .ts, .js, .mjs, .cjs).
    if let Ok(st) = SourceType::from_path(&source.id.id) {
        return st;
    }
    // Fallback based on our own module type classification.
    match source.id.module_type {
        ModuleType::Ets | ModuleType::Ts | ModuleType::Declaration => {
            SourceType::default().with_typescript(true)
        }
        _ => SourceType::default(),
    }
}

/// Extract a string from a [`ModuleExportName`] variant.
fn export_name_str(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(ident) => ident.name.as_ref().to_owned(),
        ModuleExportName::IdentifierReference(ident) => ident.name.as_ref().to_owned(),
        ModuleExportName::StringLiteral(lit) => lit.value.as_ref().to_owned(),
    }
}

/// Parse a module's source text and extract static import edges and export bindings.
///
/// Uses the ohos-rs/oxc fork parser so that `.ets` ArkTS syntax is handled correctly.
/// Returns conservative `has_side_effects = true` for any file that produces parse errors.
pub fn parse_module(source: &ModuleSource) -> ParsedModule {
    let allocator = Allocator::default();
    let source_type = source_type_for(source);

    let ParserReturn { program, errors, .. } =
        Parser::new(&allocator, &source.content, source_type).parse();

    let had_errors = !errors.is_empty();

    let mut imports: Vec<ImportEdge> = Vec::new();
    let mut exports: Vec<ExportBinding> = Vec::new();

    for stmt in &program.body {
        match stmt {
            // Static `import ... from 'specifier'`
            Statement::ImportDeclaration(decl) => {
                let specifier = decl.source.value.as_ref().to_owned();
                imports.push(ImportEdge {
                    specifier: specifier.clone(),
                    resolved: ResolvedId {
                        id: specifier,
                        external: false,
                        module_type: ModuleType::Ts,
                    },
                    imported_names: vec![],
                });
            }
            // ArkUI lazy import: `import lazy { X } from 'specifier'`
            Statement::LazyImportDeclaration(decl) => {
                let specifier = decl.source.value.as_ref().to_owned();
                imports.push(ImportEdge {
                    specifier: specifier.clone(),
                    resolved: ResolvedId {
                        id: specifier,
                        external: false,
                        module_type: ModuleType::Ts,
                    },
                    imported_names: vec![],
                });
            }
            // `export { a as b }` or `export function foo() {}`
            Statement::ExportNamedDeclaration(decl) => {
                // Explicit specifiers: `export { local as exported }`
                for spec in &decl.specifiers {
                    exports.push(ExportBinding {
                        exported_name: export_name_str(&spec.exported),
                        local_name: export_name_str(&spec.local),
                    });
                }
                // Declaration-based exports: `export function name() {}`, `export class Name {}`
                if let Some(declaration) = &decl.declaration {
                    if let Some(id) = declaration.id() {
                        let name = id.name.as_ref().to_owned();
                        exports.push(ExportBinding {
                            exported_name: name.clone(),
                            local_name: name,
                        });
                    }
                }
            }
            // `export default expr`
            Statement::ExportDefaultDeclaration(_) => {
                exports.push(ExportBinding {
                    exported_name: "default".to_owned(),
                    local_name: "default".to_owned(),
                });
            }
            // `export * from 'other'` — re-export, creates an import edge too
            Statement::ExportAllDeclaration(decl) => {
                let specifier = decl.source.value.as_ref().to_owned();
                imports.push(ImportEdge {
                    specifier: specifier.clone(),
                    resolved: ResolvedId {
                        id: specifier,
                        external: false,
                        module_type: ModuleType::Ts,
                    },
                    imported_names: vec![],
                });
                // If aliased (`export * as ns from ...`) the alias is an export binding.
                if let Some(exported) = &decl.exported {
                    exports.push(ExportBinding {
                        exported_name: export_name_str(exported),
                        local_name: "*".to_owned(),
                    });
                }
            }
            _ => {}
        }
    }

    ParsedModule {
        imports,
        exports,
        // Conservative: treat any module with parse errors as having side effects.
        has_side_effects: had_errors || true,
    }
}
