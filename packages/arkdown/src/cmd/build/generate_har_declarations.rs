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
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc_isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;
use walkdir::WalkDir;

use super::layout::ModuleBuildLayout;

pub(super) struct HarDeclarationGenerator<'a> {
  module_root: &'a Path,
  source_dir: &'a Path,
  output_prefix: PathBuf,
}

impl<'a> HarDeclarationGenerator<'a> {
  pub(super) fn new_har(module_root: &'a Path, source_dir: &'a Path) -> Self {
    Self {
      module_root,
      source_dir,
      output_prefix: PathBuf::new(),
    }
  }

  /// Hvigor's `getIntermediatesEtsTgzPath(moduleName)` places HSP
  /// declarations below `<module>/src/main/ets` before PackageSharedHar copies
  /// them to the interface HAR staging directory.
  pub(super) fn new_hsp(module_root: &'a Path, source_dir: &'a Path, module_name: &str) -> Self {
    Self {
      module_root,
      source_dir,
      output_prefix: PathBuf::from(module_name).join("src/main/ets"),
    }
  }

  pub(super) fn generate(&self) -> Result<()> {
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
      let output_path = output_dir
        .join(&self.output_prefix)
        .join(Self::declaration_relative_path(relative));
      if let Some(parent) = output_path.parent() {
        fs_err::create_dir_all(parent)?;
      }
      if arkdown_path::SourceFileName::new(source_path).is_declaration() {
        fs_err::copy(source_path, &output_path)?;
        continue;
      }
      let declaration = HarSourceDeclaration::new(source_path).emit()?;
      fs_err::write(&output_path, declaration)?;
    }
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
