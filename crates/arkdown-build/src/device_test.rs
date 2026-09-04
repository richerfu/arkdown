//! Device-test execution delegated to the SDK-owned `@ohos/coverage` package.
//!
//! This mirrors hvigor 6.24.3 `GenerateDeviceCoverage.doTaskAction`: the
//! default target and the synthetic `ohosTest` HAP are built first, then the
//! installed coverage command receives the SDK toolchains root, package paths,
//! module identity, scope, and signed HSP dependency paths.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use arkdown_core::SdkConfig;
use arkdown_session::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};

/// Failures from planning or executing the SDK-owned on-device test runner.
#[derive(Debug, Error)]
pub enum DeviceTestExecutionError {
    #[error("OHOSTEST_PATH_NOT_EXIST: onDeviceTest requires {path}")]
    MissingSourceSet { path: PathBuf },
    #[error("OHOSTEST_MODULE_JSON_NOT_EXIST: onDeviceTest requires {path}")]
    MissingModuleProfile { path: PathBuf },
    #[error("OHOSTEST_MODULE_NAME_MISSING: {path} does not declare module.name")]
    MissingModuleName { path: PathBuf },
    #[error(
    "ARKDOWN_DEVICE_TEST_COVERAGE_COMPONENT_MISSING: selected SDK has no @ohos/coverage package ({ets_loader_path})"
  )]
    MissingCoveragePackage { ets_loader_path: PathBuf },
    #[error(
    "ARKDOWN_DEVICE_TEST_TOOLCHAINS_COMPONENT_MISSING: SDK toolchains directory was not found at {path}"
  )]
    MissingToolchains { path: PathBuf },
    #[error(
    "ARKDOWN_DEVICE_TEST_HAP_MISSING: hvigor-compatible signed ohosTest HAP was not produced at {path}"
  )]
    MissingProducedTestPackage { path: PathBuf },
    #[error("ARKDOWN_DEVICE_TEST_HAP_MISSING: {path}")]
    MissingTestPackage { path: PathBuf },
    #[error(
    "ARKDOWN_DEVICE_TEST_COVERAGE_COMPONENT_MISSING: @ohos/coverage command-line API was not found at {path}"
  )]
    MissingCoverageEntrypoint { path: PathBuf },
    #[error("ARKDOWN_DEVICE_TEST_TARGET_PACKAGE_MISSING: {path}")]
    MissingTargetPackage { path: PathBuf },
    #[error("serialize @ohos/coverage device-test request: {0}")]
    RequestSerialize(#[from] serde_json::Error),
    #[error("start @ohos/coverage device-test runner failed: {0}")]
    RunnerStart(#[source] std::io::Error),
    #[error("ARKDOWN_DEVICE_TEST_RESULT_FAILED: @ohos/coverage exited with status {status}")]
    RunnerFailed { status: std::process::ExitStatus },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceTestExecutionPlan {
    toolchains_path: PathBuf,
    project_path: PathBuf,
    module_path: PathBuf,
    bundle_name: String,
    bundle_type: String,
    coverage: bool,
    scope: String,
    test_module_name: String,
    target_hap_path: PathBuf,
    test_hap_path: PathBuf,
    dependencies_paths: Vec<PathBuf>,
    #[serde(skip)]
    node_executable: PathBuf,
    #[serde(skip)]
    coverage_package_path: PathBuf,
}

pub struct DeviceTestExecutionInput<'a> {
    pub sdk_config: &'a SdkConfig,
    pub sdk_root: &'a Path,
    pub project_path: &'a Path,
    pub module_root: &'a Path,
    pub module_name: &'a str,
    pub ohos_test_module_json: Option<&'a serde_json::Value>,
    pub target_artifact_name: Option<&'a str>,
    pub product_name: &'a str,
    pub bundle_name: &'a str,
    pub bundle_type: &'a str,
    pub target_package_path: &'a Path,
    pub dependencies: &'a DependencyGraphSnapshot,
    pub coverage: bool,
    pub scope: &'a str,
    pub node_executable: PathBuf,
}

impl DeviceTestExecutionPlan {
    pub fn from_build(
        input: DeviceTestExecutionInput<'_>,
    ) -> Result<Self, DeviceTestExecutionError> {
        let ohos_test_root = input.module_root.join("src").join("ohosTest");
        if !ohos_test_root.is_dir() {
            return Err(DeviceTestExecutionError::MissingSourceSet {
                path: ohos_test_root,
            });
        }
        let module_json_path = ohos_test_root.join("module.json5");
        let module_json = input.ohos_test_module_json.ok_or_else(|| {
            DeviceTestExecutionError::MissingModuleProfile {
                path: module_json_path.clone(),
            }
        })?;
        let test_module_name = module_json
            .pointer("/module/name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or(DeviceTestExecutionError::MissingModuleName {
                path: module_json_path,
            })?
            .to_string();

        let coverage_package_path =
            input
                .sdk_config
                .hvigor_coverage_package_path()
                .ok_or_else(|| DeviceTestExecutionError::MissingCoveragePackage {
                    ets_loader_path: input.sdk_config.ets_loader_path.clone(),
                })?;
        let toolchains_path = input.sdk_root.join("toolchains");
        if !toolchains_path.is_dir() {
            return Err(DeviceTestExecutionError::MissingToolchains {
                path: toolchains_path,
            });
        }

        let test_hap_path = Self::test_hap_path(
            input.module_root,
            input.product_name,
            input.module_name,
            input.target_artifact_name,
        )?;
        let dependencies_paths =
            Self::dependency_paths(input.dependencies, input.project_path, input.product_name);

        Ok(Self {
            toolchains_path,
            project_path: arkdown_path::canonicalize_or_owned(input.project_path),
            module_path: arkdown_path::canonicalize_or_owned(input.module_root),
            bundle_name: input.bundle_name.to_string(),
            bundle_type: input.bundle_type.to_string(),
            coverage: input.coverage,
            scope: input.scope.to_string(),
            test_module_name,
            target_hap_path: input.target_package_path.to_path_buf(),
            test_hap_path,
            dependencies_paths,
            node_executable: input.node_executable,
            coverage_package_path,
        })
    }

    pub fn execute(&self) -> Result<(), DeviceTestExecutionError> {
        let command_line = self
            .coverage_package_path
            .join("lib")
            .join("src")
            .join("commandLine")
            .join("index.js");
        if !command_line.is_file() {
            return Err(DeviceTestExecutionError::MissingCoverageEntrypoint { path: command_line });
        }
        if !self.target_hap_path.is_file() {
            return Err(DeviceTestExecutionError::MissingTargetPackage {
                path: self.target_hap_path.clone(),
            });
        }
        if !self.test_hap_path.is_file() {
            return Err(DeviceTestExecutionError::MissingTestPackage {
                path: self.test_hap_path.clone(),
            });
        }

        let request = serde_json::to_string(self)?;
        let status = std::process::Command::new(&self.node_executable)
            .arg("-e")
            .arg(DEVICE_COVERAGE_COMMAND_RUNNER)
            .arg(&command_line)
            .arg(&request)
            .current_dir(&self.module_path)
            .status()
            .map_err(DeviceTestExecutionError::RunnerStart)?;
        if !status.success() {
            return Err(DeviceTestExecutionError::RunnerFailed { status });
        }
        Ok(())
    }

    fn test_hap_path(
        module_root: &Path,
        product_name: &str,
        module_name: &str,
        target_artifact_name: Option<&str>,
    ) -> Result<PathBuf, DeviceTestExecutionError> {
        let output_dir = module_root
            .join("build")
            .join(product_name)
            .join("outputs")
            .join("ohosTest");
        let file_name = Self::test_hap_file_name(module_name, target_artifact_name);
        let path = output_dir.join(file_name);
        if !path.is_file() {
            return Err(DeviceTestExecutionError::MissingProducedTestPackage { path });
        }
        Ok(path)
    }

    /// Hvigor `GenerateDeviceCoverage` calls the default target's
    /// `getModuleTargetOutputFileName(..., true, ..., "ohosTest")`.
    fn test_hap_file_name(module_name: &str, target_artifact_name: Option<&str>) -> String {
        target_artifact_name.map_or_else(
            || format!("{module_name}-ohosTest-signed.hap"),
            |artifact_name| format!("{artifact_name}.hap"),
        )
    }

    fn dependency_paths(
        dependencies: &DependencyGraphSnapshot,
        project_root: &Path,
        product_name: &str,
    ) -> Vec<PathBuf> {
        let mut seen = BTreeSet::new();
        let mut paths = Vec::new();
        for edge in &dependencies.edges {
            let Some(node) = dependencies.nodes.get(&edge.package_name) else {
                continue;
            };
            if !matches!(
                node.node_type,
                DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp
            ) {
                continue;
            }
            if let Some(path) = Self::dependency_hsp_path(node, product_name) {
                if seen.insert(path.clone()) {
                    paths.push(path);
                }
            }
        }
        for path in Self::signed_remote_hsp_paths(project_root, product_name) {
            if seen.insert(path.clone()) {
                paths.push(path);
            }
        }
        // `GenerateDeviceCoverage`: `[...getDependenciesPaths(), ...remote].reverse()`.
        paths.reverse();
        paths
    }

    fn dependency_hsp_path(node: &DependencyNodeSnapshot, product_name: &str) -> Option<PathBuf> {
        let package_root = node.package_root.as_ref()?;
        if package_root
            .extension()
            .is_some_and(|extension| extension == "hsp")
            && package_root.is_file()
        {
            return Some(package_root.clone());
        }
        let target_name = "default";
        let output_dir = package_root
            .join("build")
            .join(product_name)
            .join("outputs")
            .join(target_name);
        Self::preferred_hsp_in(&output_dir)
    }

    fn preferred_hsp_in(directory: &Path) -> Option<PathBuf> {
        let mut candidates = std::fs::read_dir(directory)
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "hsp"))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            let left_unsigned = left
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains("-unsigned"));
            let right_unsigned = right
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains("-unsigned"));
            left_unsigned
                .cmp(&right_unsigned)
                .then_with(|| left.cmp(right))
        });
        candidates.into_iter().next()
    }

    fn signed_remote_hsp_paths(project_root: &Path, product_name: &str) -> Vec<PathBuf> {
        let cache = project_root
            .join("build")
            .join("cache")
            .join(product_name)
            .join("remote_hsp")
            .join("remoteHspCache.json");
        let Ok(content) = std::fs::read_to_string(cache) else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
            return Vec::new();
        };
        value
            .get("signedRemoteHsps")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|entries| entries.values())
            .filter_map(|entry| {
                ["signedHspPath", "hspPath"]
                    .into_iter()
                    .filter_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
                    .map(PathBuf::from)
                    .find(|path| path.is_file())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceTestExecutionPlan;

    #[test]
    fn hap_name_matches_hvigor_target_output_contract() {
        assert_eq!(
            DeviceTestExecutionPlan::test_hap_file_name("entry", None),
            "entry-ohosTest-signed.hap"
        );
        assert_eq!(
            DeviceTestExecutionPlan::test_hap_file_name("entry", Some("custom-test")),
            "custom-test.hap"
        );
    }
}

const DEVICE_COVERAGE_COMMAND_RUNNER: &str = r#"
const commandLine = require(process.argv[1]);
const request = JSON.parse(process.argv[2]);
const logger = {
  debug: (...args) => process.env.ARKDOWN_LOG ? console.error(...args) : undefined,
  info: (...args) => console.error(...args),
  warn: (...args) => console.error(...args),
  error: (...args) => console.error(...args),
};
commandLine.executeCoverageOhosTest({ ...request, logger })
  .then(() => process.exit(0))
  .catch((error) => { console.error(error && error.stack || error); process.exit(1); });
"#;
