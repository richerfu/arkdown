//! SDK `import_path_expand.js::expandAllImportPaths` equivalent.
//!
//! When `arkOptions.expandImportPath.enable` is set (non-HAR modules),
//! package-level value imports are expanded into direct source-file imports,
//! skipping the package index re-export layer. Each imported symbol is traced
//! through the package entry's export graph (`export { x }`,
//! `export { x } from "./mod"`, `export * from "./mod"`, `export default`)
//! to the file that actually declares it; a new import statement is emitted
//! per declaration file.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use oxc_allocator::{Allocator, CloneIn, FromIn, GetAllocator, Vec as OxcVec};
use oxc_ast::ast::{
    ImportDeclaration, ImportDeclarationSpecifier, ModuleExportName, Program, Statement,
    StringLiteral,
};
use oxc_ast::builder::AstBuilder;
use oxc_codegen::{CodegenOptions, CommentOptions, IndentChar};
use oxc_parser::Parser;
use oxc_span::SPAN;
use oxc_str::Str;

use rustc_hash::FxHashSet;

use crate::source::{CompileArktsSourceType, TransformSourceSnapshot};
use crate::transform::source_emit::HvigorTypescriptPrinter;
use crate::transform::source_text::ViewpuOutputText;
use crate::transform::TransformOptions;

/// Parse `source`, expand package-level imports into direct source-file
/// imports, and print the result. Returns the input unchanged when expansion
/// does not apply.
/// Expand package-level imports in place on a parsed program. Runs inside the
/// transform pipeline (before codegen) so the emitted source map covers the
/// expanded imports, mirroring hvigor's `expandAllImportPaths` transformer.
pub(crate) fn apply_to_program<'a>(
    program: &mut Program<'a>,
    allocator: &'a Allocator,
    id: &'a str,
    options: &'a TransformOptions,
) -> bool {
    let Some(expand) = options.expand_import_path.as_ref() else {
        return false;
    };
    if !expand.enable {
        return false;
    }
    let rewriter = ImportPathExpander::new(
        id,
        &expand.exclude,
        &options.sources,
        &options.hsp_har_package_names,
    );
    rewriter.expand(program, allocator)
}

/// Parse `source`, expand package-level imports into direct source-file
/// imports, and print the result. Returns the input unchanged when expansion
/// does not apply.
pub(crate) fn apply<'a>(
    source: &'a str,
    id: &str,
    options: &TransformOptions,
    sources: &TransformSourceSnapshot,
) -> Cow<'a, str> {
    let Some(expand) = options.expand_import_path.as_ref() else {
        return Cow::Borrowed(source);
    };
    if !expand.enable {
        return Cow::Borrowed(source);
    }
    let allocator = Allocator::default();
    let source_type = CompileArktsSourceType::from_id(id);
    let mut parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        return Cow::Borrowed(source);
    }

    let rewriter =
        ImportPathExpander::new(id, &expand.exclude, sources, &options.hsp_har_package_names);
    if !rewriter.expand(&mut parsed.program, &allocator) {
        return Cow::Borrowed(source);
    }
    let code = HvigorTypescriptPrinter::print(
        &parsed.program,
        CodegenOptions {
            comments: CommentOptions::disabled(),
            source_map_path: None,
            indent_char: IndentChar::Space,
            indent_width: 4,
            ..CodegenOptions::default()
        },
    );
    Cow::Owned(ViewpuOutputText::new(code).without_extra_trailing_blank_lines())
}

struct ImportPathExpander<'a> {
    importer: &'a str,
    exclude: &'a [String],
    sources: &'a TransformSourceSnapshot,
    hsp_har_package_names: &'a FxHashSet<String>,
}

impl<'a> ImportPathExpander<'a> {
    fn new(
        importer: &'a str,
        exclude: &'a [String],
        sources: &'a TransformSourceSnapshot,
        hsp_har_package_names: &'a FxHashSet<String>,
    ) -> Self {
        Self {
            importer,
            exclude,
            sources,
            hsp_har_package_names,
        }
    }

    fn expand(&self, program: &mut Program<'a>, allocator: &'a Allocator) -> bool {
        let mut changed = false;
        let ast = AstBuilder::new(allocator);
        let mut body = OxcVec::with_capacity_in(program.body.len(), &ast);
        let old_body = std::mem::replace(&mut program.body, OxcVec::new_in(&ast));
        for statement in old_body {
            let Statement::ImportDeclaration(import) = &statement else {
                body.push(statement);
                continue;
            };
            let expanded = self.expand_import(import, &ast);
            if expanded.is_empty() {
                body.push(statement);
                continue;
            }
            body.extend(expanded);
            changed = true;
        }
        program.body = body;
        changed
    }

    /// `transformImportDecl`: expand one import into one import per
    /// declaration file, or an empty vec when nothing expands.
    fn expand_import<'b>(
        &self,
        import: &ImportDeclaration<'b>,
        ast: &AstBuilder<'b>,
    ) -> Vec<Statement<'b>> {
        let specifier = import.source.value.as_str();
        // Relative imports, SDK prefixed modules, SO dependencies and
        // excluded packages never expand.
        if specifier.starts_with('.')
            || specifier.contains("@kit.")
            || specifier.contains("@ohos.")
            || specifier.contains("@arkts.")
            || (specifier.starts_with("lib") && specifier.ends_with(".so"))
            || self
                .exclude
                .iter()
                .any(|excluded| excluded.as_str() == specifier)
            || self.hsp_har_package_names.contains(specifier)
        {
            return Vec::new();
        }
        if import.import_kind.is_type() {
            return Vec::new();
        }
        let Some(import_specifiers) = import.specifiers.as_ref() else {
            return Vec::new();
        };
        if import_specifiers
            .iter()
            .any(|s| matches!(s, ImportDeclarationSpecifier::ImportNamespaceSpecifier(_)))
        {
            return Vec::new();
        }

        // Package entry file for the requested package.
        let package_name = ModuleSpecifierText::new(specifier)
            .package_candidates()
            .last()
            .map(|candidate| candidate.package_name().to_string())
            .unwrap_or_else(|| specifier.to_string());
        let Some(entry) = self
            .sources
            .resolve_static_with_declarations(Path::new(self.importer), specifier)
        else {
            return Vec::new();
        };
        let entry_path = entry.path.clone();
        // Package root: the entry lives at `<pkg>/<entry>` or
        // `<pkg>/src/main/ets/<entry>`; the first ancestor holding the
        // package entry layout IS the package root (no further `parent()`).
        let package_root = entry_path
            .ancestors()
            .find(|dir| {
                dir.join("src").join("main").join("ets").is_dir()
                    || dir.join("Index.ets").is_file()
                    || dir.join("index.ets").is_file()
            })
            .map(Path::to_path_buf)
            .unwrap_or_else(|| {
                entry_path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| entry_path.clone())
            });

        // Group the imported symbols by their declaring file. The module
        // request is `@pkg/<rel>` for files below the package root; the
        // package specifier itself when the declaration lives at the entry.
        let mut groups: Vec<(String, Vec<ExpandedSpecifier<'b>>)> = Vec::new();
        // Original import span for the whole group (mapping anchor for the
        // generated imports; hvigor's `getRealFilePath` anchors the first
        // generated token at the corresponding source symbol).
        let mut group_spans: Vec<u32> = Vec::new();
        for import_specifier in import_specifiers {
            match import_specifier {
                ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => {
                    let Some((decl_file, export_name, is_default)) = self.resolve_symbol(
                        &entry_path,
                        "default",
                        true,
                        &package_name,
                        &package_root,
                    ) else {
                        continue;
                    };
                    let request = self.module_request(&decl_file, &package_name, &package_root);
                    let group = groups
                        .iter_mut()
                        .zip(group_spans.iter_mut())
                        .find(|((file, _), _)| file == &request);
                    match group {
                        Some((group, span)) => {
                            group.1.push(ExpandedSpecifier::Default {
                                local: default.local.clone_in(ast.allocator()),
                                export_name,
                                is_default,
                            });
                            *span = (*span).min(default.span.start);
                        }
                        None => {
                            group_spans.push(default.span.start);
                            groups.push((
                                request,
                                vec![ExpandedSpecifier::Default {
                                    local: default.local.clone_in(ast.allocator()),
                                    export_name,
                                    is_default,
                                }],
                            ));
                        }
                    }
                }
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    let imported_name = specifier.imported.name().as_str();
                    let Some((decl_file, export_name, is_default)) = self.resolve_symbol(
                        &entry_path,
                        imported_name,
                        false,
                        &package_name,
                        &package_root,
                    ) else {
                        continue;
                    };
                    let request = self.module_request(&decl_file, &package_name, &package_root);
                    let group = groups
                        .iter_mut()
                        .zip(group_spans.iter_mut())
                        .find(|((file, _), _)| file == &request);
                    match group {
                        Some((group, span)) => {
                            group.1.push(ExpandedSpecifier::Named {
                                local: specifier.local.clone_in(ast.allocator()),
                                imported: specifier.imported.clone_in(ast.allocator()),
                                export_name,
                                is_default,
                            });
                            *span = (*span).min(specifier.span.start);
                        }
                        None => {
                            group_spans.push(specifier.span.start);
                            groups.push((
                                request,
                                vec![ExpandedSpecifier::Named {
                                    local: specifier.local.clone_in(ast.allocator()),
                                    imported: specifier.imported.clone_in(ast.allocator()),
                                    export_name,
                                    is_default,
                                }],
                            ));
                        }
                    }
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
            }
        }
        if groups.is_empty() {
            return Vec::new();
        }

        // Emit one import declaration per declaring file.
        let mut expanded = OxcVec::with_capacity_in(groups.len(), ast);
        for ((request, specs), group_span) in groups.into_iter().zip(group_spans) {
            let import_span = oxc_span::Span::new(group_span, group_span);
            let mut specifiers = OxcVec::with_capacity_in(specs.len(), ast);
            for spec in specs {
                match spec {
                    ExpandedSpecifier::Default {
                        local,
                        export_name,
                        is_default,
                    } => {
                        if is_default {
                            specifiers.push(
                                ImportDeclarationSpecifier::new_import_default_specifier(
                                    SPAN, local, ast,
                                ),
                            );
                        } else {
                            specifiers.push(ImportDeclarationSpecifier::new_import_specifier(
                                SPAN,
                                ModuleExportName::new_identifier_name(
                                    SPAN,
                                    Str::from_in(export_name.as_str(), ast.allocator()),
                                    ast,
                                ),
                                local,
                                import.import_kind,
                                ast,
                            ));
                        }
                    }
                    ExpandedSpecifier::Named {
                        local,
                        imported,
                        is_default,
                        ..
                    } => {
                        if is_default {
                            specifiers.push(
                                ImportDeclarationSpecifier::new_import_default_specifier(
                                    SPAN, local, ast,
                                ),
                            );
                        } else {
                            specifiers.push(ImportDeclarationSpecifier::new_import_specifier(
                                SPAN,
                                imported,
                                local,
                                import.import_kind,
                                ast,
                            ));
                        }
                    }
                }
            }
            expanded.push(Statement::new_import_declaration(
                import_span,
                Some(specifiers),
                StringLiteral::new(
                    import_span,
                    Str::from_in(request.as_str(), ast.allocator()),
                    None,
                    ast,
                ),
                None,
                None,
                import.import_kind,
                ast,
            ));
        }
        expanded.into_iter().collect()
    }

    /// `getRealFilePath` equivalent: trace the symbol through the entry's
    /// export graph to its declaring file.
    fn resolve_symbol(
        &self,
        entry: &Path,
        symbol: &str,
        is_default: bool,
        package_name: &str,
        package_root: &Path,
    ) -> Option<(PathBuf, String, bool)> {
        let mut seen = FxHashSet::default();
        self.trace_export(
            entry,
            symbol,
            is_default,
            package_name,
            package_root,
            &mut seen,
        )
    }

    fn trace_export(
        &self,
        file: &Path,
        symbol: &str,
        is_default: bool,
        package_name: &str,
        package_root: &Path,
        seen: &mut FxHashSet<PathBuf>,
    ) -> Option<(PathBuf, String, bool)> {
        if !seen.insert(file.to_path_buf()) {
            return None;
        }
        let source = self.sources.source_text(file)?;
        let allocator = Allocator::default();
        let source_type = CompileArktsSourceType::from_id(&file.to_string_lossy());
        let parsed = Parser::new(&allocator, source.as_ref(), source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return None;
        }

        for statement in &parsed.program.body {
            match statement {
                // `export { x }` / `export { x as y }` — local declaration.
                Statement::ExportNamedDeclaration(export) => {
                    for element in &export.specifiers {
                        let local_name = element.local.name().as_str();
                        let exported_name = element.exported.name().as_str();
                        if exported_name == symbol {
                            return Some((file.to_path_buf(), local_name.to_string(), is_default));
                        }
                    }
                }
                // `export { x } from "./mod"` — recurse.
                Statement::ExportFromDeclaration(export) if !export.specifiers.is_empty() => {
                    let Some(resolved) = self
                        .sources
                        .resolve_relative_with_declarations(file, export.source.value.as_str())
                    else {
                        continue;
                    };
                    for element in &export.specifiers {
                        let local_name = element.local.name().as_str();
                        let exported_name = element.exported.name().as_str();
                        if exported_name == symbol {
                            return self.trace_export(
                                &resolved.path,
                                local_name,
                                false,
                                package_name,
                                package_root,
                                seen,
                            );
                        }
                    }
                }
                // `export * from "./mod"` — recurse for the symbol.
                Statement::ExportAllDeclaration(export) => {
                    let Some(resolved) = self
                        .sources
                        .resolve_relative_with_declarations(file, export.source.value.as_str())
                    else {
                        continue;
                    };
                    if let Some(result) = self.trace_export(
                        &resolved.path,
                        symbol,
                        false,
                        package_name,
                        package_root,
                        seen,
                    ) {
                        return Some(result);
                    }
                }
                // `export default <decl>` — the default export lives here.
                Statement::ExportDefaultDeclaration(_) if is_default && symbol == "default" => {
                    return Some((file.to_path_buf(), "default".to_string(), true));
                }
                // Direct `export const/function/class foo` declarations.
                Statement::ExportDeclaration(export) => {
                    let names = declaration_names(&export.declaration);
                    if names.iter().any(|name| name == symbol) {
                        return Some((file.to_path_buf(), symbol.to_string(), is_default));
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// `genModuleRequest`: absolute path → `@pkg/<relative>` module request.
    fn module_request(&self, file: &Path, package_name: &str, package_root: &Path) -> String {
        if let Ok(relative) = file.strip_prefix(package_root) {
            let relative = ArtifactPathSerializer::path_to_slash(relative)
                .map(|path| path.into_string())
                .unwrap_or_else(|_| relative.to_string_lossy().replace('\\', "/"));
            // `transformImportDecl` strips the `.ets`/`.ts` extension from the
            // generated module request.
            let relative = relative
                .strip_suffix(".ets")
                .or_else(|| relative.strip_suffix(".ts"))
                .unwrap_or(&relative);
            return format!("{package_name}/{relative}");
        }
        package_name.to_string()
    }
}

fn declaration_names(declaration: &oxc_ast::ast::Declaration<'_>) -> Vec<String> {
    match declaration {
        oxc_ast::ast::Declaration::VariableDeclaration(variable) => variable
            .declarations
            .iter()
            .filter_map(|declarator| declarator.id.get_identifier_name())
            .map(|name| name.as_str().to_string())
            .collect(),
        oxc_ast::ast::Declaration::FunctionDeclaration(function) => function
            .name()
            .as_ref()
            .map(|name| name.as_str().to_string())
            .into_iter()
            .collect(),
        oxc_ast::ast::Declaration::ClassDeclaration(class) => class
            .id
            .as_ref()
            .map(|id| id.name.as_str().to_string())
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// One imported symbol grouped under a declaring file.
enum ExpandedSpecifier<'b> {
    Default {
        local: oxc_ast::ast::BindingIdentifier<'b>,
        export_name: String,
        is_default: bool,
    },
    Named {
        local: oxc_ast::ast::BindingIdentifier<'b>,
        imported: oxc_ast::ast::ModuleExportName<'b>,
        export_name: String,
        is_default: bool,
    },
}

use arkdown_path::{ArtifactPathSerializer, ModuleSpecifierText};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn snapshot(files: &[(&str, &str)]) -> TransformSourceSnapshot {
        let mut map = rustc_hash::FxHashMap::default();
        for (path, content) in files {
            map.insert(PathBuf::from(path), Arc::from(*content));
        }
        TransformSourceSnapshot::new(map)
    }

    fn expand(
        source: &str,
        id: &str,
        options: &TransformOptions,
        sources: &TransformSourceSnapshot,
    ) -> String {
        apply(source, id, options, sources).into_owned()
    }

    fn options() -> TransformOptions {
        TransformOptions {
            expand_import_path: Some(arkdown_core::ExpandImportPath {
                enable: true,
                exclude: Vec::new(),
            }),
            ..TransformOptions::default()
        }
    }

    #[test]
    fn expands_package_import_through_index_reexports() {
        let sources = snapshot(&[
            (
                "/project/oh_modules/@ohos/mylib/Index.ets",
                "export { foo } from './src/impl';\nexport { bar } from './src/bar';\n",
            ),
            (
                "/project/oh_modules/@ohos/mylib/src/impl.ets",
                "export function foo() {}\n",
            ),
            (
                "/project/oh_modules/@ohos/mylib/src/bar.ets",
                "export const bar = 1;\n",
            ),
        ]);
        let source = "import { foo, bar } from '@ohos/mylib';\nconsole.log(foo, bar);";
        let out = expand(
            source,
            "/project/entry/src/main/ets/Index.ets",
            &options(),
            &sources,
        );
        assert!(
            out.contains("import { foo } from \"@ohos/mylib/src/impl\""),
            "got: {out}"
        );
        assert!(
            out.contains("import { bar } from \"@ohos/mylib/src/bar\""),
            "got: {out}"
        );
        assert!(!out.contains("from '@ohos/mylib'"), "got: {out}");
    }

    #[test]
    fn expands_symbols_declared_at_the_package_entry_to_the_entry_itself() {
        // SDK `getRealFilePath`: a symbol whose declaration lives in the
        // package entry (`library/Index`) still expands — the generated
        // request is the entry itself (`@ohos/mylib/Index`), matching hvigor
        // on `export const LIB_VERSION` re-exports.
        let sources = snapshot(&[(
            "/project/oh_modules/@ohos/mylib/Index.ets",
            "export function foo() {}\n",
        )]);
        let source = "import { foo } from '@ohos/mylib';\nconsole.log(foo);";
        let out = expand(
            source,
            "/project/entry/src/main/ets/Index.ets",
            &options(),
            &sources,
        );
        assert!(
            out.contains("import { foo } from \"@ohos/mylib/Index\""),
            "got: {out}"
        );
    }

    #[test]
    fn skips_relative_sdk_and_excluded_specifiers() {
        let sources = snapshot(&[(
            "/project/oh_modules/@ohos/mylib/Index.ets",
            "export const foo = 1;\n",
        )]);
        let source = [
            "import { foo } from '@ohos/mylib';",
            "import { hilog } from '@kit.PerformanceAnalysisKit';",
            "import { local } from './local';",
            "console.log(foo, hilog, local);",
        ]
        .join("\n");
        let mut options = options();
        options.expand_import_path = Some(arkdown_core::ExpandImportPath {
            enable: true,
            exclude: vec!["@ohos/mylib".to_string()],
        });
        let out = expand(
            &source,
            "/project/entry/src/main/ets/Index.ets",
            &options,
            &sources,
        );
        assert!(
            out.contains("from '@kit.PerformanceAnalysisKit'"),
            "got: {out}"
        );
        assert!(out.contains("from './local'"), "got: {out}");
        assert!(out.contains("from '@ohos/mylib'"), "got: {out}");
    }

    #[test]
    fn disabled_option_keeps_imports_unchanged() {
        let sources = snapshot(&[(
            "/project/oh_modules/@ohos/mylib/Index.ets",
            "export { foo } from './src/impl';\n",
        )]);
        let source = "import { foo } from '@ohos/mylib';\nconsole.log(foo);";
        let out = expand(
            source,
            "/project/entry/src/main/ets/Index.ets",
            &TransformOptions::default(),
            &sources,
        );
        assert_eq!(out, source, "disabled expansion must not rewrite");
    }
}
