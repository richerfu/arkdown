use std::path::{Component, Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::{ModuleExportName, Statement};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use arkdown_core::{ExportBinding, ImportEdge, ImportKind, ModuleSource, ModuleType, ResolvedId};

pub struct ParsedModule {
    pub imports: Vec<ImportEdge>,
    pub exports: Vec<ExportBinding>,
    pub has_side_effects: bool,
}

// ---------------------------------------------------------------------------
// Path resolution helpers
// ---------------------------------------------------------------------------

/// Normalize a [`PathBuf`] by resolving `.` and `..` components without
/// touching the filesystem.
fn normalize_path(path: PathBuf) -> String {
    let mut result = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                // Don't pop past the root.
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other),
        }
    }
    result.to_string_lossy().into_owned()
}

/// Resolve an import `specifier` relative to the module that contains it
/// (`importer_id` is the absolute path of the importing file).
///
/// Returns `(resolved_id, is_external)`.
///
/// - Relative specifiers (`./` / `../`) → joined with importer's parent dir,
///   path-normalized, `is_external = false`.
/// - Bare specifiers (`@ohos/`, `@bundle:`, SDK names, etc.) → kept as-is,
///   `is_external = true`.  They are resolved at runtime by ArkRuntime.
fn resolve_import_specifier(specifier: &str, importer_id: &str) -> (String, bool) {
    if specifier.starts_with('.') {
        let importer_dir = Path::new(importer_id)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let joined = importer_dir.join(specifier);
        let normalized = normalize_path(joined);
        (normalized, false)
    } else {
        // Bare / OHM specifier — external.
        (specifier.to_owned(), true)
    }
}

/// Infer [`ModuleType`] from an absolute path's file extension.
fn module_type_from_path(path: &str) -> ModuleType {
    if path.ends_with(".ets") || path.ends_with(".d.ets") {
        ModuleType::Ets
    } else if path.ends_with(".ts") || path.ends_with(".d.ts") {
        ModuleType::Ts
    } else if path.ends_with(".json") || path.ends_with(".json5") {
        ModuleType::Json5
    } else {
        ModuleType::Js
    }
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

    let ParserReturn {
        program, errors, ..
    } = Parser::new(&allocator, &source.content, source_type).parse();

    let had_errors = !errors.is_empty();

    let mut imports: Vec<ImportEdge> = Vec::new();
    let mut exports: Vec<ExportBinding> = Vec::new();

    for stmt in &program.body {
        match stmt {
            // Static `import ... from 'specifier'`
            Statement::ImportDeclaration(decl) => {
                let specifier = decl.source.value.as_ref().to_owned();
                let (resolved_id, external) = resolve_import_specifier(&specifier, &source.id.id);
                let module_type = if external {
                    ModuleType::Ts
                } else {
                    module_type_from_path(&resolved_id)
                };
                imports.push(ImportEdge {
                    specifier,
                    resolved: ResolvedId {
                        id: resolved_id,
                        external,
                        module_type,
                        ..Default::default()
                    },
                    imported_names: vec![],
                    kind: ImportKind::Import,
                });
            }
            // ArkUI lazy import: `import lazy { X } from 'specifier'`
            Statement::LazyImportDeclaration(decl) => {
                let specifier = decl.source.value.as_ref().to_owned();
                let (resolved_id, external) = resolve_import_specifier(&specifier, &source.id.id);
                let module_type = if external {
                    ModuleType::Ts
                } else {
                    module_type_from_path(&resolved_id)
                };
                imports.push(ImportEdge {
                    specifier,
                    resolved: ResolvedId {
                        id: resolved_id,
                        external,
                        module_type,
                        ..Default::default()
                    },
                    imported_names: vec![],
                    kind: ImportKind::Import,
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
                let (resolved_id, external) = resolve_import_specifier(&specifier, &source.id.id);
                let module_type = if external {
                    ModuleType::Ts
                } else {
                    module_type_from_path(&resolved_id)
                };
                imports.push(ImportEdge {
                    specifier,
                    resolved: ResolvedId {
                        id: resolved_id,
                        external,
                        module_type,
                        ..Default::default()
                    },
                    imported_names: vec![],
                    kind: ImportKind::Import,
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
