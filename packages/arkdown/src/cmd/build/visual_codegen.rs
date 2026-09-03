//! Batch compatibility adapter for the SDK Visual/SuperVisual generator.
//!
//! The source SDK ships `codegen/codegen_ets.js` as a JavaScript module. Rust
//! owns discovery, deterministic batching, diagnostics, and the OXC insertion
//! pass; Node only evaluates that SDK module and returns structured fragments.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use arkdown_ets_transform::VisualGeneratedSource;
use arkdown_path::canonicalize_or_owned;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::Deserialize;

const VISUAL_CODEGEN_HOST: &str = r#"
const fs = require('fs');
const generator = require(process.argv[1]);
const sources = JSON.parse(fs.readFileSync(0, 'utf8'));
process.stdout.write(JSON.stringify(sources.map((source) => generator.genETS(source))));
"#;

pub(super) struct VisualCodegenAdapter<'a> {
  node_executable: &'a Path,
  codegen_module: PathBuf,
  module_roots: &'a [(String, PathBuf)],
  dependency_source_roots: &'a [PathBuf],
}

impl<'a> VisualCodegenAdapter<'a> {
  pub(super) fn new(
    node_executable: &'a Path,
    ets_loader_path: &Path,
    module_roots: &'a [(String, PathBuf)],
    dependency_source_roots: &'a [PathBuf],
  ) -> Self {
    Self {
      node_executable,
      codegen_module: ets_loader_path.join("codegen/codegen_ets.js"),
      module_roots,
      dependency_source_roots,
    }
  }

  pub(super) fn generate(&self) -> Result<FxHashMap<PathBuf, VisualGeneratedSource>> {
    let inputs = VisualInputInventory::discover(self.module_roots, self.dependency_source_roots)?;
    if inputs.is_empty() {
      return Ok(FxHashMap::default());
    }
    if !self.codegen_module.is_file() {
      anyhow::bail!(
        "ARKDOWN_VISUAL_CODEGEN_MISSING: SDK Visual generator is not available at {}",
        self.codegen_module.display()
      );
    }

    let source_text = inputs
      .items
      .par_iter()
      .map(|input| {
        fs_err::read_to_string(&input.visual_path)
          .with_context(|| format!("read Visual source {}", input.visual_path.display()))
      })
      .collect::<Result<Vec<_>>>()?;
    let request = serde_json::to_vec(&source_text)?;
    let mut child = Command::new(self.node_executable)
      .arg("-e")
      .arg(VISUAL_CODEGEN_HOST)
      .arg(&self.codegen_module)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .with_context(|| {
        format!(
          "ARKDOWN_VISUAL_CODEGEN_LAUNCH: launch {} for {}",
          self.node_executable.display(),
          self.codegen_module.display()
        )
      })?;
    child
      .stdin
      .take()
      .ok_or_else(|| anyhow::anyhow!("ARKDOWN_VISUAL_CODEGEN_STDIN: Node stdin is unavailable"))?
      .write_all(&request)
      .context("ARKDOWN_VISUAL_CODEGEN_STDIN: write Visual batch")?;
    let output = child
      .wait_with_output()
      .context("ARKDOWN_VISUAL_CODEGEN_WAIT: wait for Visual generator")?;
    if !output.status.success() {
      anyhow::bail!(
        "ARKDOWN_VISUAL_CODEGEN_FAILED: {} exited with {}; stderr: {}",
        self.codegen_module.display(),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
      );
    }
    let generated: Vec<VisualCodegenResult> =
      serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
          "ARKDOWN_VISUAL_CODEGEN_OUTPUT: decode SDK output; stderr: {}",
          String::from_utf8_lossy(&output.stderr).trim()
        )
      })?;
    if generated.len() != inputs.items.len() {
      anyhow::bail!(
        "ARKDOWN_VISUAL_CODEGEN_OUTPUT: SDK returned {} results for {} inputs",
        generated.len(),
        inputs.items.len()
      );
    }

    let mut sources = FxHashMap::default();
    for (input, generated) in inputs.items.into_iter().zip(generated) {
      if !generated.error_type.is_empty() {
        anyhow::bail!(
          "ARKDOWN_VISUAL_CODEGEN_DIAGNOSTIC: {}: {} ({})",
          input.visual_path.display(),
          generated.error_message,
          generated.error_type
        );
      }
      sources.insert(input.ets_path, generated.ets);
    }
    Ok(sources)
  }
}

#[derive(Debug)]
struct VisualInput {
  ets_path: PathBuf,
  visual_path: PathBuf,
}

#[derive(Debug, Default)]
struct VisualInputInventory {
  items: Vec<VisualInput>,
}

impl VisualInputInventory {
  fn discover(
    module_roots: &[(String, PathBuf)],
    dependency_source_roots: &[PathBuf],
  ) -> Result<Self> {
    let mut roots = BTreeSet::new();
    for (_, module_root) in module_roots {
      roots.insert(module_root.join("src/main/supervisual"));
    }
    for root in dependency_source_roots {
      for candidate in Self::dependency_visual_roots(root) {
        roots.insert(candidate);
      }
    }

    let mut items = Vec::new();
    for visual_root in roots {
      if !visual_root.is_dir() {
        continue;
      }
      let ets_root = visual_root
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Visual root has no src/main parent"))?
        .join("ets");
      for entry in walkdir::WalkDir::new(&visual_root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
      {
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("visual") {
          continue;
        }
        let relative = entry
          .path()
          .strip_prefix(&visual_root)
          .with_context(|| format!("derive Visual path relative to {}", visual_root.display()))?;
        let mut ets_path = ets_root.join(relative);
        ets_path.set_extension("ets");
        if !ets_path.is_file() {
          continue;
        }
        items.push(VisualInput {
          ets_path: canonicalize_or_owned(&ets_path),
          visual_path: canonicalize_or_owned(entry.path()),
        });
      }
    }
    items.sort_by(|left, right| {
      left
        .ets_path
        .cmp(&right.ets_path)
        .then_with(|| left.visual_path.cmp(&right.visual_path))
    });
    items.dedup_by(|left, right| left.ets_path == right.ets_path);
    Ok(Self { items })
  }

  fn dependency_visual_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![root.join("src/main/supervisual")];
    if root.file_name().is_some_and(|name| name == "ets") {
      if let Some(src_main) = root.parent() {
        roots.push(src_main.join("supervisual"));
      }
    }
    roots
  }

  fn is_empty(&self) -> bool {
    self.items.is_empty()
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisualCodegenResult {
  #[serde(default)]
  ets: VisualGeneratedSource,
  #[serde(default)]
  error_type: String,
  #[serde(default)]
  error_message: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn inventory_maps_supervisual_to_existing_ets_sources_deterministically() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let ets_root = module_root.join("src/main/ets/pages");
    let visual_root = module_root.join("src/main/supervisual/pages");
    std::fs::create_dir_all(&ets_root).expect("ets root");
    std::fs::create_dir_all(&visual_root).expect("visual root");
    let source = ets_root.join("Index.ets");
    let visual = visual_root.join("Index.visual");
    std::fs::write(&source, "@Component struct Index { build() {} }").expect("source");
    std::fs::write(&visual, "{}").expect("visual");
    std::fs::write(visual_root.join("Orphan.visual"), "{}").expect("orphan");

    let inventory = VisualInputInventory::discover(&[("entry".to_string(), module_root)], &[])
      .expect("inventory");

    assert_eq!(inventory.items.len(), 1);
    assert_eq!(inventory.items[0].ets_path, canonicalize_or_owned(&source));
    assert_eq!(
      inventory.items[0].visual_path,
      canonicalize_or_owned(&visual)
    );
  }

  #[test]
  fn sdk_javascript_boundary_is_batched_and_returns_typed_fragments() {
    if Command::new("node").arg("--version").output().is_err() {
      return;
    }
    let temp = tempfile::tempdir().expect("tempdir");
    let loader = temp.path().join("ets-loader");
    let codegen = loader.join("codegen");
    let module_root = temp.path().join("entry");
    let ets_root = module_root.join("src/main/ets/pages");
    let visual_root = module_root.join("src/main/supervisual/pages");
    std::fs::create_dir_all(&codegen).expect("codegen root");
    std::fs::create_dir_all(&ets_root).expect("ets root");
    std::fs::create_dir_all(&visual_root).expect("visual root");
    std::fs::write(
      codegen.join("codegen_ets.js"),
      "exports.genETS = source => ({ ets: { build: source }, errorType: '', errorMessage: '' });",
    )
    .expect("fake SDK generator");
    let source = ets_root.join("Index.ets");
    std::fs::write(&source, "@Component struct Index { build() {} }").expect("source");
    std::fs::write(visual_root.join("Index.visual"), "    Column()").expect("visual source");

    let generated = VisualCodegenAdapter::new(
      Path::new("node"),
      &loader,
      &[("entry".to_string(), module_root)],
      &[],
    )
    .generate()
    .expect("Visual generation");

    assert_eq!(
      generated
        .get(&canonicalize_or_owned(&source))
        .map(|fragments| fragments.build.as_str()),
      Some("    Column()")
    );
  }
}
