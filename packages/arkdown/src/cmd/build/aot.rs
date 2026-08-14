use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use super::layout::ModuleBuildLayout;

/// Owns the ArkTS AOT side artifacts emitted after `modules.abc`.
///
/// The ETS loader shipped with the 6.1.1.280 toolchain
/// (`lib/gen_aot.js::generateAot`) validates the ABC/profile inputs and then
/// creates `<module>.an` and `<module>.ai` in `an/arm64-v8a`. The shipped
/// implementation constructs, but does not execute, `ark_aot_compiler`; the
/// two zero-length files are therefore the observable Hvigor output.
pub(super) struct AotArtifactWriter {
  module_name: String,
  mode: Option<String>,
  output_dir: Option<PathBuf>,
  profile_path: Option<PathBuf>,
  expected_output_dir: PathBuf,
}

impl AotArtifactWriter {
  pub(super) fn new(
    module_name: &str,
    mode: Option<&str>,
    output_dir: Option<&str>,
    profile_path: Option<&str>,
    layout: &ModuleBuildLayout<'_>,
    coverage_mode: bool,
  ) -> Self {
    Self {
      module_name: module_name.to_string(),
      // `AbstractCompileNode.commonOption` forces an empty AOT mode whenever
      // OHOS test coverage instrumentation is active.
      mode: (!coverage_mode).then(|| mode.map(str::to_string)).flatten(),
      output_dir: output_dir.map(PathBuf::from),
      profile_path: profile_path.map(PathBuf::from),
      expected_output_dir: layout.aot_output_dir(),
    }
  }

  pub(super) fn cache_ready(&self) -> bool {
    !self.requires_compiler()
      || self
        .output_files()
        .is_some_and(|[an, ai]| an.is_file() && ai.is_file())
  }

  pub(super) fn generate(&self, modules_abc: &Path) -> Result<()> {
    if !self.requires_compiler() {
      return Ok(());
    }
    if !modules_abc.is_file() {
      bail!(
        "AOT_APP_ABC_NOT_FOUND: ArkTS AOT compilation requires {}",
        modules_abc.display()
      );
    }
    let Some(profile_path) = self.profile_path.as_deref() else {
      bail!("PARTIAL_EMPTY_PATH: partial AOT compilation requires apPath");
    };
    if !profile_path.is_file() {
      bail!(
        "PARTIAL_INVALID_PATH: partial AOT profile does not exist: {}",
        profile_path.display()
      );
    }
    let Some([an, ai]) = self.output_files() else {
      bail!(
        "AOT_OUTPUT_PATH_MISSING: partial AOT compilation has no anBuildOutPut (expected {})",
        self.expected_output_dir.display()
      );
    };

    if let Err(error) = self.write_empty_outputs(&an, &ai) {
      // `gen_aot.js` catches output creation failures and lets the module run
      // under the interpreter. Preserve that non-fatal boundary.
      tracing::warn!(
        module = %self.module_name,
        error = %error,
        "ArkTS AOT output generation failed; the module may run with an interpreter"
      );
    }
    Ok(())
  }

  fn requires_compiler(&self) -> bool {
    self.mode.as_deref() == Some("partial")
  }

  fn output_files(&self) -> Option<[PathBuf; 2]> {
    let output_dir = self.output_dir.as_ref()?;
    Some([
      output_dir.join(Path::new(&self.module_name).with_extension("an")),
      output_dir.join(Path::new(&self.module_name).with_extension("ai")),
    ])
  }

  fn write_empty_outputs(&self, an: &Path, ai: &Path) -> std::io::Result<()> {
    if let Some(parent) = an.parent() {
      std::fs::create_dir_all(parent)?;
    }
    std::fs::File::create(an)?;
    std::fs::File::create(ai)?;
    Ok(())
  }
}
