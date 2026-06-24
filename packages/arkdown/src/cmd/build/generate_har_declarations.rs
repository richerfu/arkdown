//! K4 — Generate basic `.d.ts`/`.d.ets` declaration stubs for source HAR packages.
//!
//! Scans HAR source files with OXC and emits declaration entries for exported
//! type/interface/class/function/const names, plus best-effort `export * from`
//! re-export entries so downstream consumers can resolve project layouts that
//! still depend on source modules.
//!
//! Source-HAR declaration generation.
//!
//! This is separate from hvigor's ByteCode HAR `processDeclarationEntry()`,
//! which writes `oh-package.json5#metadata.declarationEntry` OHM URLs for
//! workers, runtimeOnly sources, abilities, startup tasks, and intent entries.
//! The generator here emits declaration stubs for source HAR consumers:
//! - Reads `oh-package.json5#main` to find the entry
//! - Scans source files with OXC for exported symbols
//! - Emits `export declare …` stubs in `index.d.ts`
//! - Copies to `index.d.ets` as a duplicate (ArkTS-specific declaration)

use std::path::{Path, PathBuf};

use anyhow::Result;
use arkdown_path::{ArtifactPathSerializer, ModuleSpecifier};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
  BindingPattern, Declaration, ExportDefaultDeclarationKind, ImportOrExportKind, Program,
  Statement, TSModuleDeclarationName,
};
use oxc_ast::{AstBuilder, NONE};
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::{SourceType, SPAN};
use walkdir::WalkDir;

use super::layout::ModuleBuildLayout;

pub(super) struct HarDeclarationGenerator<'a> {
  module_root: &'a Path,
  source_dir: &'a Path,
}

impl<'a> HarDeclarationGenerator<'a> {
  pub(super) fn new(module_root: &'a Path, source_dir: &'a Path, _module_name: &'a str) -> Self {
    Self {
      module_root,
      source_dir,
    }
  }

  /// Generate basic declaration files for a source HAR.
  ///
  /// Writes:
  /// - `intermediates/har/default/index.d.ts`
  /// - `intermediates/har/default/index.d.ets`
  ///
  /// The declaration stubs re-export from each `.ets`/`.ts` file discovered
  /// recursively under `source_dir`, emitting one `export * from '...'` line
  /// per file.  Files whose names begin with `_` or that are test files
  /// (name contains `.test.` or `.spec.`) are skipped.
  pub(super) fn generate(&self) -> Result<()> {
    let module_layout = ModuleBuildLayout::new(self.module_root);
    let out_dir = module_layout.har_dir();
    std::fs::create_dir_all(&out_dir)?;

    let mut specifiers = Vec::new();
    let mut declarations = Vec::new();

    let main_entry = self.main_entry();

    if let Some(main) = &main_entry {
      specifiers.push(ModuleExportSpecifier::from_main_entry(main));
      if let Some(main_path) = self.resolve_main_entry_path(main) {
        declarations.extend(HarDeclarationAnalyzer::new(&main_path).analyze());
      }
    }

    if self.source_dir.exists() {
      // Collect the stem of the main entry so we can skip it in the
      // per-file walk (it is already emitted above).
      let main_entry_abs = main_entry.as_deref().map(|main| {
        let stripped = arkdown_path::strip_relative_prefix(main);
        self
          .source_dir
          .parent()
          .unwrap_or(self.source_dir)
          .join(stripped)
      });

      let mut all_files: Vec<PathBuf> = WalkDir::new(self.source_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
          if !e.file_type().is_file() {
            return false;
          }
          let path = e.path();
          let ext = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or_default();
          if ext != "ets" && ext != "ts" {
            return false;
          }
          // Skip declaration files (.d.ts / .d.ets).
          if arkdown_path::SourceFileName::new(path).is_declaration() {
            return false;
          }
          // Skip files whose name starts with underscore.
          let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
          if file_name.starts_with('_') {
            return false;
          }
          // Skip test/spec files.
          let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
          if stem.contains(".test") || stem.contains(".spec") {
            return false;
          }
          true
        })
        .map(|e| e.path().to_path_buf())
        .collect();
      all_files.sort();

      for file_path in &all_files {
        // Skip the main entry — already emitted.
        if let Some(ref main_abs) = main_entry_abs {
          if file_path == main_abs {
            continue;
          }
        }
        let Ok(rel) = file_path.strip_prefix(self.source_dir) else {
          continue;
        };
        // Build the specifier path: ./src/main/ets/<rel without extension>
        let rel_no_ext = rel.with_extension("");
        let specifier_path = PathBuf::from("src")
          .join("main")
          .join("ets")
          .join(&rel_no_ext);
        specifiers.push(ModuleExportSpecifier::from_relative_path(&specifier_path));
        declarations.extend(HarDeclarationAnalyzer::new(file_path).analyze());
      }
    }
    declarations.sort();
    declarations.dedup();
    let declarations = HarDeclarationEmitter::new(specifiers, declarations).emit();

    let dts_path = out_dir.join("index.d.ts");
    std::fs::write(&dts_path, &declarations)?;

    let dets_path = out_dir.join("index.d.ets");
    std::fs::write(&dets_path, &declarations)?;

    Ok(())
  }

  fn main_entry(&self) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct PkgEntry {
      #[serde(default)]
      main: Option<String>,
    }
    let content =
      std::fs::read_to_string(ModuleBuildLayout::new(self.module_root).oh_package_json5()).ok()?;
    json5::from_str::<PkgEntry>(&content)
      .ok()
      .and_then(|p| p.main)
      .filter(|m| !m.is_empty())
  }

  fn resolve_main_entry_path(&self, main: &str) -> Option<PathBuf> {
    let stripped = arkdown_path::strip_relative_prefix(main);
    let base = self.source_dir.parent().unwrap_or(self.source_dir);
    let path = base.join(stripped);
    path.exists().then_some(path)
  }
}

struct ModuleExportSpecifier {
  value: String,
}

impl ModuleExportSpecifier {
  fn new(value: impl Into<String>) -> Self {
    Self {
      value: value.into(),
    }
  }

  fn from_main_entry(value: &str) -> Self {
    let mut path = PathBuf::new();
    for component in Path::new(value).components() {
      if matches!(component, std::path::Component::CurDir) {
        continue;
      }
      path.push(component.as_os_str());
    }
    path.set_extension("");
    Self::new(ArtifactPathSerializer::rel_to_slash(&path))
  }

  fn from_relative_path(path: &Path) -> Self {
    Self::new(ModuleSpecifier::from_relative_path(path).into_string())
  }
}

struct HarDeclarationEmitter {
  specifiers: Vec<ModuleExportSpecifier>,
  declarations: Vec<HarDeclaration>,
}

impl HarDeclarationEmitter {
  fn new(specifiers: Vec<ModuleExportSpecifier>, declarations: Vec<HarDeclaration>) -> Self {
    Self {
      specifiers,
      declarations,
    }
  }

  fn emit(&self) -> String {
    let allocator = Allocator::default();
    let ast = AstBuilder::new(&allocator);
    let program = self.program(ast);
    let mut code = Codegen::new()
      .with_options(CodegenOptions::default())
      .build(&program)
      .code;
    if !self.declarations.is_empty() {
      let mut declarations = self
        .declarations
        .iter()
        .map(HarDeclaration::emit)
        .collect::<Vec<_>>()
        .join("\n");
      declarations.push('\n');
      declarations.push_str(&code);
      code = declarations;
    }
    code
  }

  fn program<'a>(&self, ast: AstBuilder<'a>) -> Program<'a> {
    ast.program(
      SPAN,
      SourceType::ts().with_module(true),
      "",
      ast.vec(),
      None,
      ast.vec(),
      ast.vec_from_iter(
        self
          .specifiers
          .iter()
          .map(|specifier| self.export_all_statement(ast, specifier)),
      ),
    )
  }

  fn export_all_statement<'a>(
    &self,
    ast: AstBuilder<'a>,
    specifier: &ModuleExportSpecifier,
  ) -> Statement<'a> {
    Statement::from(ast.module_declaration_export_all_declaration(
      SPAN,
      None,
      ast.string_literal(SPAN, ast.str(&specifier.value), None),
      NONE,
      ImportOrExportKind::Value,
    ))
  }
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct HarDeclaration {
  name: String,
  kind: HarDeclarationKind,
}

impl HarDeclaration {
  fn new(name: impl Into<String>, kind: HarDeclarationKind) -> Option<Self> {
    let name = name.into();
    HarDeclarationName::new(&name)
      .is_valid()
      .then_some(Self { name, kind })
  }

  fn emit(&self) -> String {
    match self.kind {
      HarDeclarationKind::Interface => {
        format!("export declare interface {} {{}}", self.name)
      }
      HarDeclarationKind::Type => format!("export declare type {} = unknown;", self.name),
      HarDeclarationKind::Enum => format!("export declare enum {} {{}}", self.name),
      HarDeclarationKind::Class => format!("export declare class {} {{}}", self.name),
      HarDeclarationKind::AbstractClass => {
        format!("export declare abstract class {} {{}}", self.name)
      }
      HarDeclarationKind::Function => {
        format!(
          "export declare function {}(...args: never[]): unknown;",
          self.name
        )
      }
      HarDeclarationKind::Const => format!("export declare const {}: unknown;", self.name),
      HarDeclarationKind::Namespace => {
        format!("export declare namespace {} {{}}", self.name)
      }
      HarDeclarationKind::Default => {
        "declare const _default: unknown;\nexport default _default;".to_string()
      }
    }
  }
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum HarDeclarationKind {
  Interface,
  Type,
  Enum,
  Class,
  AbstractClass,
  Namespace,
  Function,
  Const,
  Default,
}

struct HarDeclarationName<'a> {
  name: &'a str,
}

impl<'a> HarDeclarationName<'a> {
  fn new(name: &'a str) -> Self {
    Self { name }
  }

  fn is_valid(&self) -> bool {
    let mut chars = self.name.chars();
    let Some(first) = chars.next() else {
      return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
      && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
  }
}

struct HarDeclarationAnalyzer<'a> {
  path: &'a Path,
}

impl<'a> HarDeclarationAnalyzer<'a> {
  fn new(path: &'a Path) -> Self {
    Self { path }
  }

  fn analyze(&self) -> Vec<HarDeclaration> {
    let Ok(source) = std::fs::read_to_string(self.path) else {
      return Vec::new();
    };
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(self.path).unwrap_or_else(|_| SourceType::ts());
    let parsed = Parser::new(&allocator, &source, source_type.with_module(true)).parse();
    if !parsed.diagnostics.is_empty() {
      return Vec::new();
    }
    let mut declarations = Vec::new();
    for statement in &parsed.program.body {
      HarDeclarationStatement::new(statement, &mut declarations).collect();
    }
    declarations
  }
}

struct HarDeclarationStatement<'a, 'b> {
  statement: &'a Statement<'a>,
  declarations: &'b mut Vec<HarDeclaration>,
}

impl<'a, 'b> HarDeclarationStatement<'a, 'b> {
  fn new(statement: &'a Statement<'a>, declarations: &'b mut Vec<HarDeclaration>) -> Self {
    Self {
      statement,
      declarations,
    }
  }

  fn collect(&mut self) {
    match self.statement {
      Statement::ExportNamedDeclaration(export) => {
        if let Some(declaration) = &export.declaration {
          HarDeclarationCollector::new(declaration, self.declarations).collect();
        }
      }
      Statement::ExportDefaultDeclaration(export) => match &export.declaration {
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
          if let Some(id) = &class.id {
            self.push(id.name.as_str(), HarDeclarationKind::Class);
          }
          self.push("_default", HarDeclarationKind::Default);
        }
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
          if let Some(id) = &function.id {
            self.push(id.name.as_str(), HarDeclarationKind::Function);
          }
          self.push("_default", HarDeclarationKind::Default);
        }
        _ => self.push("_default", HarDeclarationKind::Default),
      },
      _ => {}
    }
  }

  fn push(&mut self, name: &str, kind: HarDeclarationKind) {
    if let Some(declaration) = HarDeclaration::new(name, kind) {
      self.declarations.push(declaration);
    }
  }
}

struct HarDeclarationCollector<'a, 'b> {
  declaration: &'a Declaration<'a>,
  declarations: &'b mut Vec<HarDeclaration>,
}

impl<'a, 'b> HarDeclarationCollector<'a, 'b> {
  fn new(declaration: &'a Declaration<'a>, declarations: &'b mut Vec<HarDeclaration>) -> Self {
    Self {
      declaration,
      declarations,
    }
  }

  fn collect(&mut self) {
    match self.declaration {
      Declaration::TSInterfaceDeclaration(declaration) => {
        self.push(declaration.id.name.as_str(), HarDeclarationKind::Interface);
      }
      Declaration::TSTypeAliasDeclaration(declaration) => {
        self.push(declaration.id.name.as_str(), HarDeclarationKind::Type);
      }
      Declaration::TSEnumDeclaration(declaration) => {
        // Use Enum (not Type) for precise stub — `export declare enum Foo {}`
        self.push(declaration.id.name.as_str(), HarDeclarationKind::Enum);
      }
      Declaration::FunctionDeclaration(declaration) => {
        if let Some(id) = &declaration.id {
          self.push(id.name.as_str(), HarDeclarationKind::Function);
        }
      }
      Declaration::ClassDeclaration(declaration) => {
        if let Some(id) = &declaration.id {
          // Distinguish abstract from concrete classes for precise stubs.
          let kind = if declaration.r#abstract {
            HarDeclarationKind::AbstractClass
          } else {
            HarDeclarationKind::Class
          };
          self.push(id.name.as_str(), kind);
        }
      }
      Declaration::TSModuleDeclaration(declaration) => {
        // Namespace declarations: `namespace Foo {}` or `module Foo {}`
        let ns_name = match &declaration.id {
          TSModuleDeclarationName::Identifier(id) => id.name.as_str(),
          TSModuleDeclarationName::StringLiteral(lit) => lit.value.as_str(),
        };
        self.push(ns_name, HarDeclarationKind::Namespace);
      }
      Declaration::VariableDeclaration(declaration) => {
        for declarator in &declaration.declarations {
          HarBindingDeclarationCollector::new(&declarator.id, self.declarations).collect();
        }
      }
      _ => {}
    }
  }

  fn push(&mut self, name: &str, kind: HarDeclarationKind) {
    if let Some(declaration) = HarDeclaration::new(name, kind) {
      self.declarations.push(declaration);
    }
  }
}

struct HarBindingDeclarationCollector<'a, 'b> {
  pattern: &'a BindingPattern<'a>,
  declarations: &'b mut Vec<HarDeclaration>,
}

impl<'a, 'b> HarBindingDeclarationCollector<'a, 'b> {
  fn new(pattern: &'a BindingPattern<'a>, declarations: &'b mut Vec<HarDeclaration>) -> Self {
    Self {
      pattern,
      declarations,
    }
  }

  fn collect(&mut self) {
    match self.pattern {
      BindingPattern::BindingIdentifier(identifier) => {
        if let Some(declaration) =
          HarDeclaration::new(identifier.name.as_str(), HarDeclarationKind::Const)
        {
          self.declarations.push(declaration);
        }
      }
      BindingPattern::ObjectPattern(pattern) => {
        for property in &pattern.properties {
          HarBindingDeclarationCollector::new(&property.value, self.declarations).collect();
        }
        if let Some(rest) = &pattern.rest {
          HarBindingDeclarationCollector::new(&rest.argument, self.declarations).collect();
        }
      }
      BindingPattern::ArrayPattern(pattern) => {
        for element in pattern.elements.iter().flatten() {
          HarBindingDeclarationCollector::new(element, self.declarations).collect();
        }
        if let Some(rest) = &pattern.rest {
          HarBindingDeclarationCollector::new(&rest.argument, self.declarations).collect();
        }
      }
      BindingPattern::AssignmentPattern(pattern) => {
        HarBindingDeclarationCollector::new(&pattern.left, self.declarations).collect();
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct HarDeclarationTestFile {
    _temp: tempfile::TempDir,
    path: PathBuf,
  }

  impl HarDeclarationTestFile {
    fn new(source: &str) -> Self {
      let temp = tempfile::tempdir().expect("tempdir");
      let path = temp.path().join("Index.ets");
      std::fs::write(&path, source).expect("write source");
      Self { _temp: temp, path }
    }

    fn declarations(&self) -> Vec<String> {
      HarDeclarationAnalyzer::new(&self.path)
        .analyze()
        .into_iter()
        .map(|declaration| declaration.emit())
        .collect()
    }
  }

  #[test]
  fn analyzer_emits_default_binding_for_anonymous_default_export() {
    let file = HarDeclarationTestFile::new("export default { value: 1 };\n");

    assert_eq!(
      file.declarations(),
      vec!["declare const _default: unknown;\nexport default _default;"]
    );
  }

  #[test]
  fn analyzer_keeps_named_default_class_and_default_binding() {
    let file = HarDeclarationTestFile::new("export default class Entry {}\n");
    let mut declarations = file.declarations();
    declarations.sort();

    assert_eq!(
      declarations,
      vec![
        "declare const _default: unknown;\nexport default _default;",
        "export declare class Entry {}",
      ]
    );
  }
}
