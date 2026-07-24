use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use arkdown_core::CopyCodeResource;

/// Copies non-code files from ArkTS source roots into the loader output.
///
/// This mirrors `copy-resources-util.ts#getCopyOptions`: configured includes
/// are authoritative; otherwise ETS/TS/JS files and configured excludes are
/// ignored. The sibling `src/main/share` tree always uses the source-extension
/// filter from hvigor.
pub(super) struct CodeResourceCopyPlan {
  source_roots: Vec<PathBuf>,
  shared_root: PathBuf,
  output_root: PathBuf,
  manifest_path: PathBuf,
  options: CopyCodeResource,
}

impl CodeResourceCopyPlan {
  pub(super) fn new(
    source_roots: Vec<PathBuf>,
    shared_root: PathBuf,
    output_root: PathBuf,
    manifest_path: PathBuf,
    options: CopyCodeResource,
  ) -> Self {
    Self {
      source_roots,
      shared_root,
      output_root,
      manifest_path,
      options,
    }
  }

  pub(super) fn execute(&self) -> Result<Vec<PathBuf>> {
    let previous = self.read_previous_manifest()?;
    let mut current = BTreeSet::new();

    if self.options.enable {
      for source_root in &self.source_roots {
        self.copy_source_root(source_root, &mut current)?;
      }
      self.copy_shared_root(&mut current)?;
    }

    for stale in previous.difference(&current) {
      let stale_path = self.output_root.join(stale);
      if stale_path.is_file() {
        fs_err::remove_file(&stale_path)
          .with_context(|| format!("remove stale code resource {}", stale_path.display()))?;
      }
    }
    self.write_manifest(&current)?;
    Ok(
      current
        .into_iter()
        .map(|relative| self.output_root.join(relative))
        .collect(),
    )
  }

  fn copy_source_root(&self, source_root: &Path, copied: &mut BTreeSet<PathBuf>) -> Result<()> {
    if !source_root.is_dir() {
      return Ok(());
    }
    for entry in walkdir::WalkDir::new(source_root)
      .follow_links(false)
      .min_depth(1)
      .into_iter()
      .filter_map(|entry| entry.ok())
      .filter(|entry| entry.file_type().is_file())
    {
      let source = entry.path();
      if !self.should_copy_source(source) {
        continue;
      }
      let relative = source
        .strip_prefix(source_root)
        .with_context(|| format!("derive code resource path for {}", source.display()))?;
      self.copy_file(source, relative, copied)?;
    }
    Ok(())
  }

  fn copy_shared_root(&self, copied: &mut BTreeSet<PathBuf>) -> Result<()> {
    if !self.shared_root.is_dir() {
      return Ok(());
    }
    for entry in walkdir::WalkDir::new(&self.shared_root)
      .follow_links(false)
      .min_depth(1)
      .into_iter()
      .filter_map(|entry| entry.ok())
      .filter(|entry| entry.file_type().is_file())
    {
      let source = entry.path();
      if Self::is_code_source(source) {
        continue;
      }
      let relative = PathBuf::from("..").join("share").join(
        source
          .strip_prefix(&self.shared_root)
          .with_context(|| format!("derive shared code resource path for {}", source.display()))?,
      );
      self.copy_file(source, &relative, copied)?;
    }
    Ok(())
  }

  fn should_copy_source(&self, source: &Path) -> bool {
    let candidate = Self::match_path(source);
    if let Some(includes) = self.options.includes.as_ref() {
      return includes
        .iter()
        .any(|pattern| fast_glob::glob_match(Self::normalized_pattern(pattern), &candidate));
    }
    if Self::is_code_source(source) {
      return false;
    }
    !self
      .options
      .excludes
      .iter()
      .any(|pattern| fast_glob::glob_match(Self::normalized_pattern(pattern), &candidate))
  }

  fn copy_file(
    &self,
    source: &Path,
    relative: &Path,
    copied: &mut BTreeSet<PathBuf>,
  ) -> Result<()> {
    let destination = self.output_root.join(relative);
    if let Some(parent) = destination.parent() {
      fs_err::create_dir_all(parent)
        .with_context(|| format!("create code resource directory {}", parent.display()))?;
    }
    fs_err::copy(source, &destination).with_context(|| {
      format!(
        "copy code resource {} to {}",
        source.display(),
        destination.display()
      )
    })?;
    copied.insert(relative.to_path_buf());
    Ok(())
  }

  fn read_previous_manifest(&self) -> Result<BTreeSet<PathBuf>> {
    if !self.manifest_path.is_file() {
      return Ok(BTreeSet::new());
    }
    let content = fs_err::read_to_string(&self.manifest_path)?;
    let entries: Vec<String> = serde_json::from_str(&content)
      .with_context(|| format!("parse {}", self.manifest_path.display()))?;
    Ok(entries.into_iter().map(PathBuf::from).collect())
  }

  fn write_manifest(&self, copied: &BTreeSet<PathBuf>) -> Result<()> {
    if let Some(parent) = self.manifest_path.parent() {
      fs_err::create_dir_all(parent)?;
    }
    let entries: Vec<String> = copied
      .iter()
      .map(|path| path.to_string_lossy().replace('\\', "/"))
      .collect();
    fs_err::write(&self.manifest_path, serde_json::to_vec_pretty(&entries)?)?;
    Ok(())
  }

  fn is_code_source(path: &Path) -> bool {
    matches!(
      path.extension().and_then(|extension| extension.to_str()),
      Some("ets" | "ts" | "js")
    )
  }

  fn match_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
  }

  fn normalized_pattern(pattern: &str) -> String {
    pattern.replace('\\', "/")
  }
}
