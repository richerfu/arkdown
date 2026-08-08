//! Source HAR declaration generation.
//!
//! Hvigor delegates declaration emission to the SDK TypeScript/ArkTS compiler
//! and writes the result under `loader_out/<target>/etsFortgz`, preserving the
//! source tree relative to `src/main/ets`. Arkdown uses OXC's isolated
//! declarations transform from the same OHOS OXC fork used by the main ArkTS
//! pipeline. This preserves public signatures, class members, type aliases,
//! imports, exports, overloads and JSDoc instead of synthesizing placeholder
//! `unknown` declarations.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions, IndentChar};
use oxc_isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use walkdir::WalkDir;

use super::layout::ModuleBuildLayout;

pub(super) struct HarDeclarationGenerator<'a> {
  module_root: &'a Path,
  source_dir: &'a Path,
  module_output_prefix: PathBuf,
  output_prefix: PathBuf,
  generated_sources: Vec<&'a Path>,
}

impl<'a> HarDeclarationGenerator<'a> {
  pub(super) fn new_har(module_root: &'a Path, source_dir: &'a Path, module_name: &str) -> Self {
    let module_output_prefix = PathBuf::from(module_name);
    Self {
      module_root,
      source_dir,
      output_prefix: module_output_prefix.join("src/main/ets"),
      module_output_prefix,
      generated_sources: Vec::new(),
    }
  }

  /// Hvigor's `getIntermediatesEtsTgzPath(moduleName)` places HSP
  /// declarations below `<module>/src/main/ets` before PackageSharedHar copies
  /// them to the interface HAR staging directory.
  pub(super) fn new_hsp(module_root: &'a Path, source_dir: &'a Path, module_name: &str) -> Self {
    let module_output_prefix = PathBuf::from(module_name);
    Self {
      module_root,
      source_dir,
      output_prefix: module_output_prefix.join("src/main/ets"),
      module_output_prefix,
      generated_sources: Vec::new(),
    }
  }

  /// The latest `ets-loader` declaration pass also emits declarations for
  /// generated TypeScript compile entries. ByteCode HAR's `ModuleInfo.ts` is
  /// the concrete example: `generateSourceFilesInHar` preserves its path
  /// relative to the consuming module below `etsFortgz`.
  pub(super) fn with_generated_source(mut self, source: &'a Path) -> Self {
    self.generated_sources.push(source);
    self
  }

  pub(super) fn generate(&self) -> Result<()> {
    tracing::debug!(source_dir = %self.source_dir.display(), "HSP declaration generation started");
    if !self.source_dir.is_dir() {
      anyhow::bail!(
        "Source HAR declaration root does not exist: {}",
        self.source_dir.display()
      );
    }
    let output_dir = ModuleBuildLayout::new(self.module_root).declarations_dir();
    if output_dir.exists() {
      fs_err::remove_dir_all(&output_dir).with_context(|| {
        format!(
          "remove stale Source HAR declarations at {}",
          output_dir.display()
        )
      })?;
    }
    fs_err::create_dir_all(&output_dir)?;

    let mut sources = WalkDir::new(self.source_dir)
      .min_depth(1)
      .into_iter()
      .collect::<std::result::Result<Vec<_>, _>>()?;
    sources.retain(|entry| entry.file_type().is_file() && Self::is_typescript_source(entry.path()));
    sources.sort_by(|left, right| left.path().cmp(right.path()));

    for entry in sources {
      let source_path = entry.path();
      let relative = source_path.strip_prefix(self.source_dir).with_context(|| {
        format!(
          "declaration source {} is outside {}",
          source_path.display(),
          self.source_dir.display()
        )
      })?;
      self.emit_source(
        source_path,
        output_dir.join(&self.output_prefix).join(relative),
      )?;
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
      self.emit_source(
        source_path,
        output_dir.join(&self.module_output_prefix).join(relative),
      )?;
    }
    Ok(())
  }

  fn emit_source(&self, source_path: &Path, output_source_path: PathBuf) -> Result<()> {
    let output_path = Self::declaration_relative_path(&output_source_path);
    if let Some(parent) = output_path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    if arkdown_path::SourceFileName::new(source_path).is_declaration() {
      fs_err::copy(source_path, &output_path)?;
      return Ok(());
    }
    let declaration = HarSourceDeclaration::new(source_path).emit()?;
    fs_err::write(&output_path, declaration)?;
    Ok(())
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
}

impl<'a> HarSourceDeclaration<'a> {
  fn new(path: &'a Path) -> Self {
    Self { path }
  }

  fn emit(&self) -> Result<String> {
    let source = fs_err::read_to_string(self.path)
      .with_context(|| format!("read Source HAR declaration input {}", self.path.display()))?;
    let source_type = SourceType::from_path(self.path)
      .unwrap_or_else(|_| SourceType::ts())
      .with_module(true);
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
    // OXC's IsolatedDeclarations does not lower ArkUI `struct` statements;
    // the SDK TypeScript compiler emits `declare struct` with decorators and
    // member signatures. Emit that shape directly for struct-only sources
    // (widget form pages), falling back to IsolatedDeclarations otherwise.
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
    if !structs.is_empty() {
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
      out.push_str(&format!("declare struct {} {{\r\n", struct_statement.id.name));
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
              out.push_str(&source[first.pattern.span()].trim());
              for parameter in params.iter().skip(1) {
                out.push_str(", ");
                out.push_str(&source[parameter.pattern.span()].trim());
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

