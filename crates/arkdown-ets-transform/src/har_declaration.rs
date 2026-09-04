//! Source HAR and HSP declaration generation.
//!
//! The current `ets2bundle` owner is `compiler/src/ets_checker.ts`'s
//! `processHarSourceFiles`, which delegates each source to
//! `compiler/src/utils.ts::generateSourceFilesInHar`. The caller supplies the
//! resolved `etsFortgz` output directory so this domain owner does not read
//! build profiles or depend on application-layer layouts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use arkdown_core::ArktsLanguageVersion;
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions, IndentChar};
use oxc_isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions};
use oxc_parser::Parser;
use oxc_span::GetSpan;
use walkdir::WalkDir;

use crate::{declaration_merge::DeclarationMerger, CompileArktsSourceType};

pub struct HarDeclarationGenerator<'a> {
    module_root: &'a Path,
    source_dir: &'a Path,
    output_dir: PathBuf,
    module_output_prefix: PathBuf,
    output_prefix: PathBuf,
    generated_sources: Vec<&'a Path>,
    bundled_entries: Vec<PathBuf>,
    dependency_packages: BTreeMap<String, (PathBuf, PathBuf)>,
    arkts_language_version: ArktsLanguageVersion,
}

impl<'a> HarDeclarationGenerator<'a> {
    pub fn new(
        module_root: &'a Path,
        source_dir: &'a Path,
        output_dir: PathBuf,
        module_name: &str,
    ) -> Self {
        let module_output_prefix = PathBuf::from(module_name);
        Self {
            module_root,
            source_dir,
            output_dir,
            output_prefix: module_output_prefix.join("src/main/ets"),
            module_output_prefix,
            generated_sources: Vec::new(),
            bundled_entries: Vec::new(),
            dependency_packages: BTreeMap::new(),
            arkts_language_version: ArktsLanguageVersion::default(),
        }
    }

    /// The latest `ets-loader` declaration pass also emits declarations for
    /// generated TypeScript compile entries. ByteCode HAR's `ModuleInfo.ts` is
    /// the concrete example: `generateSourceFilesInHar` preserves its path
    /// relative to the consuming module below `etsFortgz`.
    pub fn with_generated_source(mut self, source: &'a Path) -> Self {
        self.generated_sources.push(source);
        self
    }

    pub fn with_arkts_language_version(mut self, version: ArktsLanguageVersion) -> Self {
        self.arkts_language_version = version;
        self
    }

    /// Enable SDK `arkOptions.bundle.bundledDeclare` for the package's public
    /// module-relative entry paths (`main` or `oh-exports`).
    pub fn with_bundled_declaration_entries<I, P>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.bundled_entries = entries.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_package_bundled_declarations(
        self,
        enabled: bool,
        main: Option<&str>,
        oh_exports: Option<&[String]>,
    ) -> Self {
        if !enabled {
            return self;
        }
        if let Some(exports) = oh_exports.filter(|exports| !exports.is_empty()) {
            return self.with_bundled_declaration_entries(exports.iter().map(PathBuf::from));
        }
        match main.filter(|main| !main.is_empty()) {
            Some(main) => self.with_bundled_declaration_entries([PathBuf::from(main)]),
            None => self,
        }
    }

    /// Add package declaration roots from the already-resolved dependency
    /// graph. Collection happens only for bundled declarations.
    pub fn with_dependency_declaration_packages<I, S, P, E>(mut self, packages: I) -> Self
    where
        I: IntoIterator<Item = (S, P, E)>,
        S: Into<String>,
        P: Into<PathBuf>,
        E: Into<PathBuf>,
    {
        self.dependency_packages = packages
            .into_iter()
            .map(|(name, root, entry)| {
                let root = root.into();
                let entry = Self::declaration_relative_path(&root.join(entry.into()));
                (name.into(), (root, entry))
            })
            .collect();
        self
    }

    pub fn generate(&self) -> Result<()> {
        tracing::debug!(source_dir = %self.source_dir.display(), "HSP declaration generation started");
        if !self.source_dir.is_dir() {
            anyhow::bail!(
                "Source HAR declaration root does not exist: {}",
                self.source_dir.display()
            );
        }
        let output_dir = &self.output_dir;
        if output_dir.exists() {
            fs_err::remove_dir_all(output_dir).with_context(|| {
                format!(
                    "remove stale Source HAR declarations at {}",
                    output_dir.display()
                )
            })?;
        }
        fs_err::create_dir_all(output_dir)?;

        let mut sources = WalkDir::new(self.source_dir)
            .min_depth(1)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        sources.retain(|entry| {
            entry.file_type().is_file() && Self::is_typescript_source(entry.path())
        });
        sources.sort_by(|left, right| left.path().cmp(right.path()));

        let mut declarations = BTreeMap::new();
        for entry in sources {
            let source_path = entry.path();
            let relative = source_path.strip_prefix(self.source_dir).with_context(|| {
                format!(
                    "declaration source {} is outside {}",
                    source_path.display(),
                    self.source_dir.display()
                )
            })?;
            let (path, declaration) = self.emit_source(
                source_path,
                output_dir.join(&self.output_prefix).join(relative),
            )?;
            declarations.insert(path, declaration);
        }

        for source_path in &self.generated_sources {
            let relative = source_path
                .strip_prefix(self.module_root)
                .with_context(|| {
                    format!(
                        "generated declaration source {} is outside module root {}",
                        source_path.display(),
                        self.module_root.display()
                    )
                })?;
            let (path, declaration) = self.emit_source(
                source_path,
                output_dir.join(&self.module_output_prefix).join(relative),
            )?;
            declarations.insert(path, declaration);
        }

        if !self.bundled_entries.is_empty() {
            for (root, _) in self.dependency_packages.values() {
                if !root.is_dir() {
                    continue;
                }
                let mut dependency_declarations = WalkDir::new(root)
                    .min_depth(1)
                    .into_iter()
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                dependency_declarations.retain(|entry| {
                    entry.file_type().is_file()
                        && arkdown_path::SourceFileName::new(entry.path()).is_declaration()
                });
                dependency_declarations.sort_by(|left, right| left.path().cmp(right.path()));
                for entry in dependency_declarations {
                    declarations.insert(
                        entry.path().to_path_buf(),
                        fs_err::read_to_string(entry.path())?,
                    );
                }
            }
            let entries = self
                .bundled_entries
                .iter()
                .map(|entry| {
                    output_dir
                        .join(&self.module_output_prefix)
                        .join(Self::declaration_relative_path(entry))
                })
                .collect::<Vec<_>>();
            declarations = DeclarationMerger::new(declarations)
                .with_package_declarations(
                    self.dependency_packages
                        .iter()
                        .map(|(name, (root, entry))| (name.clone(), root.clone(), entry.clone())),
                )
                .merge(&entries)?
                .files;
        }

        for (path, declaration) in declarations {
            if let Some(parent) = path.parent() {
                fs_err::create_dir_all(parent)?;
            }
            fs_err::write(path, declaration)?;
        }
        Ok(())
    }

    fn emit_source(
        &self,
        source_path: &Path,
        output_source_path: PathBuf,
    ) -> Result<(PathBuf, String)> {
        let output_path = Self::declaration_relative_path(&output_source_path);
        if arkdown_path::SourceFileName::new(source_path).is_declaration() {
            return Ok((output_path, fs_err::read_to_string(source_path)?));
        }
        let declaration =
            HarSourceDeclaration::new(source_path, self.arkts_language_version).emit()?;
        Ok((output_path, declaration))
    }

    fn is_typescript_source(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts")
        )
    }

    fn declaration_relative_path(source: &Path) -> PathBuf {
        let extension = match source.extension().and_then(|extension| extension.to_str()) {
            Some("ets") => "d.ets",
            _ => "d.ts",
        };
        source.with_extension(extension)
    }
}

struct HarSourceDeclaration<'a> {
    path: &'a Path,
    arkts_language_version: ArktsLanguageVersion,
}

impl<'a> HarSourceDeclaration<'a> {
    fn new(path: &'a Path, arkts_language_version: ArktsLanguageVersion) -> Self {
        Self {
            path,
            arkts_language_version,
        }
    }

    fn emit(&self) -> Result<String> {
        let source = fs_err::read_to_string(self.path).with_context(|| {
            format!("read Source HAR declaration input {}", self.path.display())
        })?;
        let source_type =
            CompileArktsSourceType::from_path_with_version(self.path, self.arkts_language_version);
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            anyhow::bail!(
                "parse Source HAR declaration input {} failed:\n{}",
                self.path.display(),
                parsed
                    .diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        // ArkTS 1.1 structs use the legacy declaration shape below. The current
        // OXC static frontend owns ArkTS 1.2 struct declarations, including
        // static-only modifiers, overloads, accessors and package declarations.
        let structs = parsed
            .program
            .body
            .iter()
            .filter_map(|statement| match statement {
                oxc_ast::ast::Statement::StructStatement(struct_statement) => {
                    Some(struct_statement.as_ref())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if !self.arkts_language_version.is_static() && !structs.is_empty() {
            return Self::emit_struct_declarations(&source, &structs);
        }
        let declarations = IsolatedDeclarations::new(
            &allocator,
            IsolatedDeclarationsOptions {
                strip_internal: false,
            },
        )
        .build(&parsed.program);
        if !declarations.diagnostics.is_empty() {
            anyhow::bail!(
                "emit Source HAR declarations for {} failed:\n{}",
                self.path.display(),
                declarations
                    .diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        let code = Codegen::new()
            .with_options(CodegenOptions {
                // TypeScript's declaration printer used by the latest SDK emits four
                // spaces, while OXC defaults to tab indentation. These bytes are
                // packaged verbatim in ByteCode HAR's interface HAR.
                indent_char: IndentChar::Space,
                indent_width: 4,
                comments: CommentOptions {
                    jsdoc: true,
                    ..CommentOptions::disabled()
                },
                ..CodegenOptions::default()
            })
            .build(&declarations.program)
            .code;
        // The SDK declaration emitter uses CRLF on every host and
        // PackageSharedHar copies those bytes without normalization.
        Ok(code.replace('\n', "\r\n"))
    }
}

impl<'a> HarSourceDeclaration<'a> {
    /// SDK `declare struct` emission for ArkUI struct sources: decorators on
    /// their own lines, `declare struct <name>`, members with signatures and
    /// no initializers, methods as `name(params): returnType;`.
    fn emit_struct_declarations(
        source: &str,
        structs: &[&oxc_ast::ast::StructStatement<'_>],
    ) -> Result<String> {
        let mut out = String::new();
        for struct_statement in structs {
            for decorator in &struct_statement.decorators {
                let expression = &decorator.expression;
                out.push_str(&format!("@{}\r\n", source[expression.span()].trim()));
            }
            out.push_str(&format!(
                "declare struct {} {{\r\n",
                struct_statement.id.name
            ));
            for element in &struct_statement.body.body {
                match element {
                    oxc_ast::ast::StructElement::PropertyDefinition(property) => {
                        for decorator in &property.decorators {
                            let expression = &decorator.expression;
                            out.push_str(&format!("    @{}\r\n", source[expression.span()].trim()));
                        }
                        out.push_str("    ");
                        if property.readonly {
                            out.push_str("readonly ");
                        }
                        let key = match &property.key {
                            oxc_ast::ast::PropertyKey::StaticIdentifier(identifier) => {
                                identifier.name.as_str().to_string()
                            }
                            oxc_ast::ast::PropertyKey::StringLiteral(literal) => {
                                literal.value.as_str().to_string()
                            }
                            other => source[other.span()].trim().to_string(),
                        };
                        out.push_str(&key);
                        if let Some(type_annotation) = &property.type_annotation {
                            out.push_str(": ");
                            out.push_str(source[type_annotation.type_annotation.span()].trim());
                        }
                        out.push_str(";\r\n");
                    }
                    oxc_ast::ast::StructElement::MethodDefinition(method) => {
                        let key = match &method.key {
                            oxc_ast::ast::PropertyKey::StaticIdentifier(identifier) => {
                                identifier.name.as_str().to_string()
                            }
                            other => source[other.span()].trim().to_string(),
                        };
                        out.push_str(&format!("    {key}"));
                        out.push('(');
                        let params = &method.value.params.items;
                        if let Some(first) = params.first() {
                            out.push_str(source[first.pattern.span()].trim());
                            for parameter in params.iter().skip(1) {
                                out.push_str(", ");
                                out.push_str(source[parameter.pattern.span()].trim());
                            }
                        }
                        out.push(')');
                        if let Some(return_type) = &method.value.return_type {
                            out.push_str(": ");
                            out.push_str(source[return_type.type_annotation.span()].trim());
                        } else {
                            // SDK declaration output defaults unannotated methods to void.
                            out.push_str(": void");
                        }
                        out.push_str(";\r\n");
                    }
                    _ => {}
                }
            }
            out.push_str("}\r\n");
        }
        Ok(out)
    }
}
