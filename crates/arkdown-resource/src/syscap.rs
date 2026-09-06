use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use thiserror::Error;

pub struct SyscapTransformInput<'a> {
    pub runtime_os: &'a str,
    pub module_name: &'a str,
    pub compile_api_version: u32,
    pub tool_path: &'a Path,
    pub output_dir: PathBuf,
    pub is_har: bool,
    /// `None` means the module declares no device types. `Some(empty)` means it
    /// declares device types that resolve to no SDK capabilities.
    pub device_capabilities: Option<BTreeSet<String>>,
    /// Parsed `src/main/syscap.json` captured by `BuildSession`.
    pub custom_syscap: Option<&'a Value>,
}

/// Executes hvigor's OpenHarmony `SyscapTransform` contract for one module.
/// HarmonyOS targets and HAR modules intentionally produce no artifact.
pub struct SyscapTransform<'a> {
    input: SyscapTransformInput<'a>,
}

impl<'a> SyscapTransform<'a> {
    pub fn new(input: SyscapTransformInput<'a>) -> Self {
        Self { input }
    }

    pub fn run(&self) -> Result<Option<PathBuf>, SyscapTransformError> {
        if self.input.runtime_os == "HarmonyOS" || self.input.is_har {
            return Ok(None);
        }
        if !self.input.tool_path.is_file() {
            return Err(SyscapTransformError::MissingTool {
                module_name: self.input.module_name.to_string(),
                path: self.input.tool_path.to_path_buf(),
            });
        }
        let capabilities = self.resolve_capabilities()?;
        if capabilities.is_empty() {
            return Err(SyscapTransformError::NoCapabilities {
                module_name: self.input.module_name.to_string(),
            });
        }
        fs_err::create_dir_all(&self.input.output_dir).map_err(|source| {
            SyscapTransformError::Io {
                operation: "create output directory",
                path: self.input.output_dir.clone(),
                source,
            }
        })?;
        let rpcid_json = self.input.output_dir.join("rpcid.json");
        let rpcid_sc = self.input.output_dir.join("rpcid.sc");
        let rpcid = serde_json::to_vec(&self.rpcid_value(&capabilities)).map_err(|source| {
            SyscapTransformError::Json {
                module_name: self.input.module_name.to_string(),
                source,
            }
        })?;
        fs_err::write(&rpcid_json, rpcid).map_err(|source| SyscapTransformError::Io {
            operation: "write rpcid input",
            path: rpcid_json.clone(),
            source,
        })?;
        let output = Command::new(self.input.tool_path)
            .args(["-R", "-e", "-i"])
            .arg(&rpcid_json)
            .arg("-o")
            .arg(&self.input.output_dir)
            .output()
            .map_err(|source| SyscapTransformError::Execute {
                path: self.input.tool_path.to_path_buf(),
                source,
            })?;
        if !output.status.success() {
            return Err(SyscapTransformError::ToolFailed {
                module_name: self.input.module_name.to_string(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        if !rpcid_sc.is_file() {
            return Err(SyscapTransformError::MissingOutput { path: rpcid_sc });
        }
        Ok(Some(rpcid_sc))
    }

    fn rpcid_value(&self, capabilities: &BTreeSet<String>) -> Value {
        serde_json::json!({
          // `developtools/syscap_codec/src/syscap_tool.c::FillOsCapLength`
          // reads this exact snake-case field from the RPCID input.
          "api_version": self.input.compile_api_version,
          "syscap": capabilities,
        })
    }

    fn resolve_capabilities(&self) -> Result<BTreeSet<String>, SyscapTransformError> {
        let general = self.input.device_capabilities.clone().unwrap_or_default();
        let Some(value) = self.input.custom_syscap else {
            return Ok(general);
        };
        let custom = Self::custom_capabilities(value);
        let mut capabilities = if self.input.device_capabilities.is_none() {
            if custom.is_empty() {
                return Err(SyscapTransformError::MissingCustomCapabilities {
                    module_name: self.input.module_name.to_string(),
                });
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

#[derive(Debug, Error)]
pub enum SyscapTransformError {
    #[error("SDK syscap_tool does not exist for module '{module_name}': {path}")]
    MissingTool { module_name: String, path: PathBuf },
    #[error("module '{module_name}' has no valid system capabilities")]
    NoCapabilities { module_name: String },
    #[error("module '{module_name}' must define devices.custom when deviceTypes is empty")]
    MissingCustomCapabilities { module_name: String },
    #[error("failed to {operation} at {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("serialize rpcid input for module '{module_name}': {source}")]
    Json {
        module_name: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to execute SDK syscap_tool {path}: {source}")]
    Execute {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("syscap_tool failed for module '{module_name}': {stdout}{stderr}")]
    ToolFailed {
        module_name: String,
        stdout: String,
        stderr: String,
    },
    #[error("syscap_tool succeeded without producing {path}")]
    MissingOutput { path: PathBuf },
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

    #[test]
    fn resolved_capabilities_use_session_snapshots_and_production_overrides() {
        let custom = serde_json::json!({
          "devices": {
            "custom": [
              { "phone": ["Common", "Phone"] },
              { "tablet": ["Common", "Tablet"] }
            ]
          },
          "production": {
            "addedSysCaps": ["ProductionOnly"],
            "removedSysCaps": ["Common"]
          }
        });
        let transform = SyscapTransform::new(SyscapTransformInput {
            runtime_os: "OpenHarmony",
            module_name: "entry",
            compile_api_version: 12,
            tool_path: Path::new("syscap_tool"),
            output_dir: PathBuf::from("syscap"),
            is_har: false,
            device_capabilities: Some(BTreeSet::from([
                "Common".to_string(),
                "SdkOnly".to_string(),
            ])),
            custom_syscap: Some(&custom),
        });

        assert_eq!(
            transform.resolve_capabilities().unwrap(),
            BTreeSet::from(["ProductionOnly".to_string()])
        );
    }

    #[test]
    fn rpcid_input_matches_current_syscap_tool_schema() {
        let transform = SyscapTransform::new(SyscapTransformInput {
            runtime_os: "OpenHarmony",
            module_name: "entry",
            compile_api_version: 26,
            tool_path: Path::new("syscap_tool"),
            output_dir: PathBuf::from("syscap"),
            is_har: false,
            device_capabilities: None,
            custom_syscap: None,
        });
        let rpcid = transform.rpcid_value(&BTreeSet::from(["SystemCapability.Base".to_string()]));

        assert_eq!(rpcid.get("api_version"), Some(&Value::from(26)));
        assert!(rpcid.get("apiVersion").is_none());
        assert_eq!(
            rpcid.get("syscap"),
            Some(&serde_json::json!(["SystemCapability.Base"]))
        );
    }
}
