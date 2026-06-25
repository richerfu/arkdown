//! `filesInfo.txt` manifest writer for `es2abc`.
//!
//! hvigor generates a `filesInfo.txt` file before invoking `es2abc`.  Each
//! line describes one source file with 7 semicolon-separated columns:
//!
//! ```text
//! <path>;<record>;<module_type>;<source_file>;<pkg_name>;<is_shared>;<lang>
//! ```
//!
//! The `--inputFileList @filesInfo.txt` flag passes this list to `es2abc`.
//!
//! For non-merge mode, the 4th column is the **per-input output path** for
//! the individual `.abc` file instead of the source-map attribution string.
//! This is a SPEC-049 known gap that is tracked here.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `FileInfoLine` | one row of `filesInfo.txt` |
//! | `FileInfoColumnPolicy` | merge vs non-merge column semantics |
//! | `CacheFileManifestWriter` | `--cache-file` `@modules.cache` argument |

use std::path::{Path, PathBuf};

use arkdown_path::ArtifactPathSerializer;

use crate::cmd::build::bytecode::error::Ts2abcError;
use crate::cmd::build::bytecode::plan::CompileUnitPlan;

// ---------------------------------------------------------------------------
// FileInfoColumnPolicy
// ---------------------------------------------------------------------------

/// Policy governing which value goes in column 4 of `filesInfo.txt`.
///
/// In merge mode, column 4 is the source-file attribution string (the
/// hvigor-scoped `"module|pkg|version|src/main/ets/..."` path used for
/// source-map and debug attribution).
///
/// In non-merge mode, column 4 is the **output path** for the per-file
/// `.abc` output.  This is a distinction tracked as a SPEC-049 parity gap.
///
/// # Upstream correspondence
///
/// Corresponds to `es2panda/aot/options.cpp::CollectInputFilesFromFileList`
/// 7-column format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileInfoColumnPolicy {
  /// Merge mode: column 4 = source-file attribution string.
  MergeColumns,
  /// Non-merge mode: column 4 = per-input output `.abc` path.
  NonMergeColumns,
}

impl FileInfoColumnPolicy {
  /// Return the column 4 value for the given compile unit.
  pub fn column4_for(&self, unit: &CompileUnitPlan) -> Result<String, Ts2abcError> {
    match self {
      FileInfoColumnPolicy::MergeColumns => unit
        .source_file
        .clone()
        .map(Ok)
        .unwrap_or_else(|| unit.source_path_str()),
      FileInfoColumnPolicy::NonMergeColumns => unit
        .output_path
        .as_deref()
        .map(Ts2abcPathText::new)
        .map(Ts2abcPathText::string)
        .unwrap_or_else(|| unit.source_path_str()),
    }
  }

  /// Return `MergeColumns` if `merge_abc` is true, else `NonMergeColumns`.
  pub fn merge_columns(merge_abc: bool) -> Self {
    if merge_abc {
      FileInfoColumnPolicy::MergeColumns
    } else {
      FileInfoColumnPolicy::NonMergeColumns
    }
  }
}

// Helper extension for CompileUnitPlan to get the source path as a String.
impl CompileUnitPlan {
  pub(crate) fn source_path_str(&self) -> Result<String, Ts2abcError> {
    match &self.source {
      crate::cmd::build::bytecode::plan::ModuleSourceInput::File { path } => {
        Ts2abcPathText::new(path).string()
      }
      crate::cmd::build::bytecode::plan::ModuleSourceInput::AbcFile { path } => {
        Ts2abcPathText::new(path).string()
      }
    }
  }
}

// ---------------------------------------------------------------------------
// FileInfoLine
// ---------------------------------------------------------------------------

/// A single row in `filesInfo.txt`.
///
/// # Upstream correspondence
///
/// Corresponds to one file line in the `filesInfo.txt` passed to
/// `es2panda/aot/options.cpp::CollectInputFilesFromFileList`.
#[derive(Debug, Clone)]
pub struct FileInfoLine {
  /// Column 1: source file path.
  pub path: String,
  /// Column 2: ABC record name.
  pub record_name: String,
  /// Column 3: module type (`"esm"` or `"commonjs"`).
  pub module_type: String,
  /// Column 4: source-file attribution OR per-input output path.
  pub col4: String,
  /// Column 5: package name.
  pub package_name: String,
  /// Column 6: `true`/`false` — whether this is a shared HSP module.
  pub is_shared_module: bool,
  /// Column 7: original source language (`"ets"`, `"ts"`, `"js"`).
  pub origin_source_lang: String,
  /// Existing bytecode input line (`abcPath;;;;pkgName;`) instead of a source
  /// compilation row.
  pub is_abc_input: bool,
}

pub(crate) struct FileInfoInput<'a> {
  pub(crate) path: &'a Path,
  pub(crate) record_name: &'a str,
  pub(crate) source_file: Option<&'a str>,
  pub(crate) package_name: Option<&'a str>,
  pub(crate) is_commonjs: bool,
  pub(crate) force_commonjs: bool,
  pub(crate) is_shared_module: bool,
  pub(crate) origin_source_lang: &'a str,
}

impl FileInfoLine {
  /// Construct a `FileInfoLine` from a `CompileUnitPlan`.
  pub fn from_compile_unit(
    unit: &CompileUnitPlan,
    policy: FileInfoColumnPolicy,
  ) -> Result<Self, Ts2abcError> {
    if matches!(
      unit.source,
      crate::cmd::build::bytecode::plan::ModuleSourceInput::AbcFile { .. }
    ) {
      return Ok(Self {
        path: unit.source_path_str()?,
        record_name: String::new(),
        module_type: String::new(),
        col4: String::new(),
        package_name: unit.package_name.clone().unwrap_or_default(),
        is_shared_module: false,
        origin_source_lang: String::new(),
        is_abc_input: true,
      });
    }
    Ok(Self {
      path: unit.source_path_str()?,
      record_name: unit.record_name.resolve(),
      module_type: FileInfoModuleType::new(unit.is_commonjs)
        .as_str()
        .to_string(),
      col4: policy.column4_for(unit)?,
      package_name: unit.package_name.clone().unwrap_or_default(),
      is_shared_module: unit.is_shared_module,
      origin_source_lang: unit.origin_source_lang.clone(),
      is_abc_input: false,
    })
  }

  pub(crate) fn from_input(input: FileInfoInput<'_>) -> Result<Self, Ts2abcError> {
    let is_abc_input = input.path.extension().and_then(|value| value.to_str()) == Some("abc");
    Ok(Self {
      path: Ts2abcPathText::new(input.path).string()?,
      record_name: if is_abc_input {
        String::new()
      } else {
        input.record_name.to_string()
      },
      module_type: if is_abc_input {
        String::new()
      } else {
        FileInfoModuleType::new(input.is_commonjs || input.force_commonjs)
          .as_str()
          .to_string()
      },
      col4: if is_abc_input {
        String::new()
      } else {
        input
          .source_file
          .map(ToOwned::to_owned)
          .map(Ok)
          .unwrap_or_else(|| Ts2abcPathText::new(input.path).string())?
      },
      package_name: input.package_name.unwrap_or_default().to_string(),
      is_shared_module: !is_abc_input && input.is_shared_module,
      origin_source_lang: if is_abc_input {
        String::new()
      } else {
        input.origin_source_lang.to_string()
      },
      is_abc_input,
    })
  }

  /// Serialize to the upstream 7-column semicolon-separated format.
  ///
  /// # Upstream correspondence
  ///
  /// Matches the exact column order from
  /// `es2panda/aot/options.cpp::CollectInputFilesFromFileList`.
  pub fn to_upstream_line(&self) -> String {
    if self.is_abc_input {
      return format!("{};;;;{};", self.path, self.package_name);
    }
    format!(
      "{};{};{};{};{};{};{}",
      self.path,
      self.record_name,
      self.module_type,
      self.col4,
      self.package_name,
      self.is_shared_module,
      self.origin_source_lang
    )
  }
}

struct FileInfoModuleType {
  is_commonjs: bool,
}

impl FileInfoModuleType {
  fn new(is_commonjs: bool) -> Self {
    Self { is_commonjs }
  }

  fn as_str(&self) -> &'static str {
    if self.is_commonjs {
      "commonjs"
    } else {
      "esm"
    }
  }
}

pub(crate) struct FileInfoManifestPath<'a> {
  output_dir: &'a Path,
  base_stem: &'a str,
}

impl<'a> FileInfoManifestPath<'a> {
  pub(crate) fn from_output(output: &'a Path) -> Self {
    Self {
      output_dir: output.parent().unwrap_or_else(|| Path::new(".")),
      base_stem: output
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("modules"),
    }
  }

  pub(crate) fn path(&self) -> PathBuf {
    self
      .output_dir
      .join(PathBuf::from(self.base_stem).with_extension("filesInfo.txt"))
  }
}

pub(crate) struct FileInfoManifestText {
  content: String,
}

impl FileInfoManifestText {
  pub(crate) fn new() -> Self {
    Self {
      content: String::new(),
    }
  }

  pub(crate) fn push_line(&mut self, line: &FileInfoLine) {
    self.content.push_str(&line.to_upstream_line());
    self.content.push('\n');
  }

  pub(crate) fn into_string(self) -> String {
    self.content
  }
}

// ---------------------------------------------------------------------------
// CacheFileManifestWriter
// ---------------------------------------------------------------------------

/// Writes the `modules.cache` manifest file for `--cache-file @` mode.
///
/// The Panda ProgramCache uses a cache manifest file that the `@` prefix
/// reads back as a file list.  `CacheFileManifestWriter` generates this file.
///
/// # Upstream correspondence
///
/// Corresponds to the cache file argument (`--cache-file @modules.cache`)
/// in `driver.rs::cache_file_arg` and the ProgramCache path convention in
/// `es2panda/aot/options.cpp`.
pub struct CacheFileManifestWriter {
  output_dir: PathBuf,
  cache_file_name: String,
}

struct CacheManifestLine<'a> {
  source_path: &'a str,
  cache_path: &'a str,
}

impl<'a> CacheManifestLine<'a> {
  fn new(source_path: &'a str, cache_path: &'a str) -> Self {
    Self {
      source_path,
      cache_path,
    }
  }

  fn to_upstream_line(&self) -> String {
    [self.source_path, self.cache_path].join(";")
  }
}

impl CacheFileManifestWriter {
  /// Create a writer that will write `<name>` in `output_dir`.
  pub fn new(output_dir: impl Into<PathBuf>, cache_file_name: impl Into<String>) -> Self {
    Self {
      output_dir: output_dir.into(),
      cache_file_name: cache_file_name.into(),
    }
  }

  /// The path of the cache file this writer will produce.
  pub fn cache_file_path(&self) -> PathBuf {
    self.output_dir.join(&self.cache_file_name)
  }

  /// Write the cache manifest listing per-input cache file paths.
  ///
  /// Each line in the cache manifest is `<source_path>;<cache_path>`.
  /// Hvigor also appends the npm module entry list itself so es2abc emits
  /// `npmEntries.protoBin` and merges native module entry records.
  pub fn write_with_npm_entries(
    &self,
    units: &[CompileUnitPlan],
    npm_entries_path: Option<&Path>,
  ) -> Result<PathBuf, Ts2abcError> {
    let path = self.cache_file_path();
    fs_err::create_dir_all(&self.output_dir).map_err(|e| Ts2abcError::Io(e.into()))?;
    let mut content = String::new();
    for unit in units {
      if let Some(cache_file) = &unit.cache_file {
        let source_path = unit.source_path_str()?;
        let cache_path = Ts2abcPathText::new(cache_file).string()?;
        content.push_str(&CacheManifestLine::new(&source_path, &cache_path).to_upstream_line());
        content.push('\n');
      }
    }
    if let Some(npm_entries_path) = npm_entries_path {
      let source_path = Ts2abcPathText::new(npm_entries_path).string()?;
      let cache_path =
        Ts2abcPathText::new(&npm_entries_path.with_extension("protoBin")).string()?;
      content.push_str(&CacheManifestLine::new(&source_path, &cache_path).to_upstream_line());
      content.push('\n');
    }
    fs_err::write(&path, &content).map_err(|e| Ts2abcError::Io(e.into()))?;
    Ok(path)
  }
}

pub(crate) struct Ts2abcPathText<'a> {
  path: &'a Path,
}

impl<'a> Ts2abcPathText<'a> {
  pub(crate) fn new(path: &'a Path) -> Self {
    Self { path }
  }

  pub(crate) fn string(self) -> Result<String, Ts2abcError> {
    Ok(ArtifactPathSerializer::path_to_slash_string(self.path)?)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cmd::build::bytecode::plan::{ModuleSourceInput, RecordNamePlan};
  use std::path::PathBuf;

  fn make_unit() -> CompileUnitPlan {
    CompileUnitPlan {
      source: ModuleSourceInput::File {
        path: PathBuf::from("/src/main/ets/pages/Index.ets"),
      },
      record_name: RecordNamePlan::Explicit("entry/pages/Index".to_string()),
      source_file: Some("entry|pkg|1.0.0|src/main/ets/pages/Index.ts".to_string()),
      package_name: Some("entry".to_string()),
      is_commonjs: false,
      is_shared_module: false,
      origin_source_lang: "ets".to_string(),
      output_path: None,
      cache_file: None,
    }
  }

  #[test]
  fn file_info_line_merge_mode() {
    let unit = make_unit();
    let line = FileInfoLine::from_compile_unit(&unit, FileInfoColumnPolicy::MergeColumns).unwrap();
    let serialized = line.to_upstream_line();
    // col1=path, col2=record, col3=module_type, col4=source_file, col5=pkg, col6=false, col7=lang
    assert!(serialized.contains("entry/pages/Index"));
    assert!(serialized.contains("esm"));
    assert!(serialized.contains("entry|pkg|1.0.0|src/main/ets/pages/Index.ts"));
    assert!(serialized.contains("entry"));
    assert!(serialized.contains("ets"));
  }

  #[test]
  fn file_info_line_non_merge_uses_output_path() {
    let mut unit = make_unit();
    unit.output_path = Some(PathBuf::from("/out/Index.abc"));
    let line =
      FileInfoLine::from_compile_unit(&unit, FileInfoColumnPolicy::NonMergeColumns).unwrap();
    let serialized = line.to_upstream_line();
    assert!(serialized.contains("/out/Index.abc"));
  }

  #[test]
  fn column_policy_merge_false_is_non_merge() {
    assert_eq!(
      FileInfoColumnPolicy::merge_columns(false),
      FileInfoColumnPolicy::NonMergeColumns
    );
    assert_eq!(
      FileInfoColumnPolicy::merge_columns(true),
      FileInfoColumnPolicy::MergeColumns
    );
  }

  #[test]
  fn cache_manifest_writer_outputs_upstream_readable_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut unit = make_unit();
    let cache_file = dir.path().join("Index.protoBin");
    unit.cache_file = Some(cache_file.clone());
    let npm_entries = dir.path().join("npmEntries.txt");

    let writer = CacheFileManifestWriter::new(dir.path(), "modules.cache");
    let manifest = writer
      .write_with_npm_entries(&[unit], Some(&npm_entries))
      .unwrap();
    let content = fs_err::read_to_string(&manifest).unwrap();
    let npm_entries_proto = npm_entries.with_extension("protoBin");
    let npm_entries_key = Ts2abcPathText::new(&npm_entries).string().unwrap();
    let cache_file_text = Ts2abcPathText::new(&cache_file).string().unwrap();
    let npm_entries_proto_text = Ts2abcPathText::new(&npm_entries_proto).string().unwrap();

    assert!(content.contains(&format!("/src/main/ets/pages/Index.ets;{cache_file_text}")));
    assert!(content.contains(&format!("{npm_entries_key};{npm_entries_proto_text}")));
  }
}
