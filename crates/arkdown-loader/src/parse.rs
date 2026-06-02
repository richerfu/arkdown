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

impl ParsedModule {
    /// Parse a module's source text and extract static import edges and export bindings.
    ///
    /// Uses the ohos-rs/oxc fork parser so that `.ets` ArkTS syntax is handled correctly.
    /// Returns conservative `has_side_effects = true` for any file that produces parse errors.
    pub fn parse(source: &ModuleSource) -> Self {
        ModuleParser::new(source).parse()
    }
}


struct ModuleParser<'a> {
    source: &'a ModuleSource,
    imports: Vec<ImportEdge>,
    exports: Vec<ExportBinding>,
}

impl<'a> ModuleParser<'a> {
    fn new(source: &'a ModuleSource) -> Self {
        Self {
            source,
            imports: Vec::new(),
            exports: Vec::new(),
        }
    }

    fn parse(mut self) -> ParsedModule {
        let allocator = Allocator::default();
        let source_type = self.source_type();

        let ParserReturn {
            program, errors, ..
        } = Parser::new(&allocator, &self.source.content, source_type).parse();

        for stmt in &program.body {
            self.visit_statement(stmt);
        }

        ParsedModule {
            imports: self.imports,
            exports: self.exports,
            // Conservative: treat any module with parse errors as having side effects.
            has_side_effects: !errors.is_empty() || true,
        }
    }

    fn visit_statement(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::ImportDeclaration(decl) => {
                self.push_import(decl.source.value.as_ref(), ImportKind::Import);
            }
            Statement::LazyImportDeclaration(decl) => {
                self.push_import(decl.source.value.as_ref(), ImportKind::Import);
            }
            Statement::ExportNamedDeclaration(decl) => {
                for spec in &decl.specifiers {
                    self.exports.push(ExportBinding {
                        exported_name: Self::export_name_str(&spec.exported),
                        local_name: Self::export_name_str(&spec.local),
                    });
                }
                if let Some(declaration) = &decl.declaration {
                    if let Some(id) = declaration.id() {
                        let name = id.name.as_ref().to_owned();
                        self.exports.push(ExportBinding {
                            exported_name: name.clone(),
                            local_name: name,
                        });
                    }
                }
            }
            Statement::ExportDefaultDeclaration(_) => {
                self.exports.push(ExportBinding {
                    exported_name: "default".to_owned(),
                    local_name: "default".to_owned(),
                });
            }
            Statement::ExportAllDeclaration(decl) => {
                self.push_import(decl.source.value.as_ref(), ImportKind::Import);
                if let Some(exported) = &decl.exported {
                    self.exports.push(ExportBinding {
                        exported_name: Self::export_name_str(exported),
                        local_name: "*".to_owned(),
                    });
                }
            }
            _ => {}
        }
    }

    fn push_import(&mut self, specifier: &str, kind: ImportKind) {
        let (resolved_id, external) = self.resolve_import_specifier(specifier);
        let module_type = if external {
            ModuleType::Ts
        } else {
            Self::module_type_from_path(&resolved_id)
        };
        self.imports.push(ImportEdge {
            specifier: specifier.to_owned(),
            resolved: ResolvedId {
                id: resolved_id,
                external,
                module_type,
                ..Default::default()
            },
            imported_names: vec![],
            kind,
        });
    }

    /// Resolve an import specifier relative to this module without touching the
    /// filesystem. Full OHOS resolution is handled later by `arkdown-resolver`.
    fn resolve_import_specifier(&self, specifier: &str) -> (String, bool) {
        if specifier.starts_with('.') {
            let importer_dir = Path::new(&self.source.id.id)
                .parent()
                .unwrap_or_else(|| Path::new(""));
            let normalized = Self::normalize_path(importer_dir.join(specifier));
            (normalized, false)
        } else {
            (specifier.to_owned(), true)
        }
    }

    /// Infer [`SourceType`] from OXC's path support, then from our module type.
    fn source_type(&self) -> SourceType {
        if let Ok(st) = SourceType::from_path(&self.source.id.id) {
            return st;
        }
        match self.source.id.module_type {
            ModuleType::Ets | ModuleType::Ts | ModuleType::Declaration => {
                SourceType::default().with_typescript(true)
            }
            _ => SourceType::default(),
        }
    }

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

    fn normalize_path(path: PathBuf) -> String {
        let mut result = PathBuf::new();
        for comp in path.components() {
            match comp {
                Component::ParentDir => {
                    result.pop();
                }
                Component::CurDir => {}
                other => result.push(other),
            }
        }
        result.to_string_lossy().into_owned()
    }

    fn export_name_str(name: &ModuleExportName<'_>) -> String {
        match name {
            ModuleExportName::IdentifierName(ident) => ident.name.as_ref().to_owned(),
            ModuleExportName::IdentifierReference(ident) => ident.name.as_ref().to_owned(),
            ModuleExportName::StringLiteral(lit) => lit.value.as_ref().to_owned(),
        }
    }
}

/// Parse a module's source text and extract static import edges and export bindings.
///
/// Prefer [`ParsedModule::parse`] in new code.
pub fn parse_module(source: &ModuleSource) -> ParsedModule {
    ParsedModule::parse(source)
}
