use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PkgInfo {
  pub package_name: String,
  pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileContextInfo {
  pub path: Option<PathBuf>,
  pub compile_entries: Vec<String>,
  pub hsp_pkg_names: BTreeSet<String>,
  pub pkg_context_info: BTreeMap<String, PkgInfo>,
  pub update_version_info: BTreeMap<String, BTreeMap<String, PkgInfo>>,
  pub need_modify_record: bool,
  pub bundle_name: String,
}

impl CompileContextInfo {
  pub fn read_upstream_lenient(path: &Path) -> io::Result<Self> {
    let mut info = Self {
      path: Some(path.to_path_buf()),
      ..Self::default()
    };
    let Ok(buffer) = std::fs::read_to_string(path) else {
      return Ok(info);
    };
    if buffer.is_empty() {
      return Ok(info);
    }
    let Ok(json) = serde_json::from_str::<Value>(&buffer) else {
      return Ok(info);
    };

    let Some(compile_entries) = json.get("compileEntries").and_then(Value::as_array) else {
      return Ok(info);
    };
    let Some(hsp_pkg_names) = json.get("hspPkgNames").and_then(Value::as_array) else {
      return Ok(info);
    };

    info.compile_entries = compile_entries
      .iter()
      .filter_map(|value| value.as_str().map(str::to_string))
      .collect();
    info.hsp_pkg_names = hsp_pkg_names
      .iter()
      .filter_map(|value| value.as_str().map(str::to_string))
      .collect();
    if let Some(value) = json.get("needModifyRecord").and_then(Value::as_bool) {
      info.need_modify_record = value;
    }
    if let Some(value) = json.get("bundleName").and_then(Value::as_str) {
      info.bundle_name = value.to_string();
    }
    parse_update_version_info(&json, &mut info);
    Ok(info)
  }
}

fn parse_update_version_info(json: &Value, info: &mut CompileContextInfo) {
  if let Some(update_version_info) = json.get("updateVersionInfo").and_then(Value::as_object) {
    for (abc_name, version_info) in update_version_info {
      let Some(version_info) = version_info.as_object() else {
        continue;
      };
      let mut pkg_context = BTreeMap::new();
      for (pkg_name, version) in version_info {
        let Some(version) = version.as_str() else {
          continue;
        };
        pkg_context.insert(
          pkg_name.clone(),
          PkgInfo {
            package_name: pkg_name.clone(),
            version: version.to_string(),
          },
        );
      }
      info
        .update_version_info
        .insert(abc_name.clone(), pkg_context);
    }
    return;
  }

  if let Some(pkg_context_info) = json.get("pkgContextInfo").and_then(Value::as_object) {
    for (pkg_name, pkg_context) in pkg_context_info {
      let Some(pkg_context) = pkg_context.as_object() else {
        continue;
      };
      let package_name = pkg_context
        .get("packageName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
      let version = pkg_context
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
      info.pkg_context_info.insert(
        pkg_name.clone(),
        PkgInfo {
          package_name,
          version,
        },
      );
    }
  }
}
