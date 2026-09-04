use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;

/// Executes hvigor's OpenHarmony `SyscapTransform` contract for one module.
/// HarmonyOS targets and HAR modules intentionally produce no artifact.
pub(super) struct SyscapTransform<'a> {
  runtime_os: &'a str,
  module_name: &'a str,
  module_root: &'a Path,
  device_types: &'a [String],
  compile_api_version: u32,
  sdk_root: &'a Path,
  tool_path: &'a Path,
  output_dir: PathBuf,
  is_har: bool,
}

impl<'a> SyscapTransform<'a> {
  #[allow(clippy::too_many_arguments)]
  pub(super) fn new(
    runtime_os: &'a str,
    module_name: &'a str,
    module_root: &'a Path,
    device_types: &'a [String],
    compile_api_version: u32,
    sdk_root: &'a Path,
    tool_path: &'a Path,
    output_dir: PathBuf,
    is_har: bool,
  ) -> Self {
    Self {
      runtime_os,
      module_name,
      module_root,
      device_types,
      compile_api_version,
      sdk_root,
      tool_path,
      output_dir,
      is_har,
    }
  }

  pub(super) fn run(&self) -> Result<Option<PathBuf>> {
    if self.runtime_os == "HarmonyOS" || self.is_har {
      return Ok(None);
    }
    if !self.tool_path.is_file() {
      anyhow::bail!(
        "SDK syscap_tool does not exist for module '{}': {}",
        self.module_name,
        self.tool_path.display()
      );
    }
    let capabilities = self.resolve_capabilities()?;
    if capabilities.is_empty() {
      anyhow::bail!(
        "module '{}' has no valid system capabilities for deviceTypes {:?}",
        self.module_name,
        self.device_types
      );
    }
    fs_err::create_dir_all(&self.output_dir)?;
    let rpcid_json = self.output_dir.join("rpcid.json");
    let rpcid_sc = self.output_dir.join("rpcid.sc");
    fs_err::write(
      &rpcid_json,
      serde_json::to_vec(&serde_json::json!({
        "apiVersion": self.compile_api_version,
        "syscap": capabilities,
      }))?,
    )?;
    let output = Command::new(self.tool_path)
      .args(["-R", "-e", "-i"])
      .arg(&rpcid_json)
      .arg("-o")
      .arg(&self.output_dir)
      .output()
      .with_context(|| format!("failed to execute {}", self.tool_path.display()))?;
    if !output.status.success() {
      anyhow::bail!(
        "syscap_tool failed for module '{}': {}{}",
        self.module_name,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
      );
    }
    if !rpcid_sc.is_file() {
      anyhow::bail!(
        "syscap_tool succeeded without producing {}",
        rpcid_sc.display()
      );
    }
    Ok(Some(rpcid_sc))
  }

  fn resolve_capabilities(&self) -> Result<BTreeSet<String>> {
    let general = self.general_capabilities()?;
    let syscap_path = self.module_root.join("src/main/syscap.json");
    if !syscap_path.is_file() {
      return Ok(general);
    }
    let value: Value = json5::from_str(&fs_err::read_to_string(&syscap_path)?)
      .with_context(|| format!("parse {}", syscap_path.display()))?;
    let custom = Self::custom_capabilities(&value);
    let mut capabilities = if self.device_types.is_empty() {
      if custom.is_empty() {
        anyhow::bail!(
          "{} must define devices.custom when module deviceTypes is empty",
          syscap_path.display()
        );
      }
      custom
    } else if custom.is_empty() {
      general
    } else {
      general.intersection(&custom).cloned().collect()
    };
    if let Some(production) = value.get("production") {
      for capability in production
        .get("addedSysCaps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
      {
        capabilities.insert(capability.to_string());
      }
      for capability in production
        .get("removedSysCaps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
      {
        capabilities.remove(capability);
      }
    }
    Ok(capabilities)
  }

  fn general_capabilities(&self) -> Result<BTreeSet<String>> {
    let device_define = if self.sdk_root.join("ets").is_dir() {
      self.sdk_root.join("ets/api/device-define")
    } else {
      self.sdk_root.join("js/api/device-define")
    };
    let mut sets = Vec::new();
    for device_type in self.device_types {
      let path = device_define.join(format!("{device_type}.json"));
      if !path.is_file() {
        tracing::warn!(
          module = self.module_name,
          device_type,
          path = %path.display(),
          "SDK system-capability definition is missing"
        );
        sets.push(BTreeSet::new());
        continue;
      }
      let value: Value = json5::from_str(&fs_err::read_to_string(&path)?)
        .with_context(|| format!("parse SDK syscap definition {}", path.display()))?;
      sets.push(
        value
          .get("SysCaps")
          .and_then(Value::as_array)
          .into_iter()
          .flatten()
          .filter_map(Value::as_str)
          .map(str::to_string)
          .collect::<BTreeSet<_>>(),
      );
    }
    let mut sets = sets.into_iter();
    let Some(mut intersection) = sets.next() else {
      return Ok(BTreeSet::new());
    };
    for capabilities in sets {
      intersection = intersection.intersection(&capabilities).cloned().collect();
    }
    Ok(intersection)
  }

  fn custom_capabilities(value: &Value) -> BTreeSet<String> {
    let sets = value
      .pointer("/devices/custom")
      .and_then(Value::as_array)
      .into_iter()
      .flatten()
      .flat_map(|device| {
        device
          .as_object()
          .into_iter()
          .flat_map(|object| object.values())
      })
      .filter_map(Value::as_array)
      .map(|capabilities| {
        capabilities
          .iter()
          .filter_map(Value::as_str)
          .map(str::to_string)
          .collect::<BTreeSet<_>>()
      })
      .collect::<Vec<_>>();
    let mut sets = sets.into_iter();
    let Some(mut intersection) = sets.next() else {
      return BTreeSet::new();
    };
    for capabilities in sets {
      intersection = intersection.intersection(&capabilities).cloned().collect();
    }
    intersection
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn custom_capabilities_intersect_devices_like_hvigor() {
    let value = serde_json::json!({
      "devices": {
        "custom": [
          { "phone": ["A", "B"] },
          { "tablet": ["B", "C"] }
        ]
      }
    });
    assert_eq!(
      SyscapTransform::custom_capabilities(&value),
      BTreeSet::from(["B".to_string()])
    );
  }
}
