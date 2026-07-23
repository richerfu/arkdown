use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_core::SdkConfig;

use crate::error::ConfigError;

/// Metadata overrides from hvigor's project cache.
///
/// `target_api_version` controls the compile target passed to tools.  The
/// optional `compile_sdk_version` only overrides the value written into
/// generated profiles. It does not select a different SDK component path.
#[derive(Debug, Clone, Default)]
pub struct SdkSelection {
    pub target_api_version: Option<u32>,
    pub compile_sdk_version: Option<String>,
}

// ---------------------------------------------------------------------------
// SDK path discovery
// ---------------------------------------------------------------------------

/// Return candidate SDK root paths to try in priority order.
///
/// Order:
/// 1. `DEVECO_SDK_HOME` environment variable (hvigor HarmonyOS SDK root)
/// 2. `OHOS_BASE_SDK_HOME` environment variable (hvigor OpenHarmony SDK root)
/// 3. `OHOS_SDK_HOME` environment variable
/// 4. `~/Library/Huawei/Sdk/` (macOS)
/// 5. `~/AppData/Local/Huawei/Sdk/` (Windows)
/// 6. `/opt/huawei/sdk/` (Linux)
fn sdk_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // hvigor reads DEVECO_SDK_HOME for HarmonyOS SDK discovery.
    if let Ok(home) = std::env::var("DEVECO_SDK_HOME") {
        candidates.push(PathBuf::from(home));
    }

    // hvigor reads OHOS_BASE_SDK_HOME for OpenHarmony SDK discovery.
    if let Ok(home) = std::env::var("OHOS_BASE_SDK_HOME") {
        candidates.push(PathBuf::from(home));
    }

    if let Ok(home) = std::env::var("OHOS_SDK_HOME") {
        candidates.push(PathBuf::from(home));
    }

    // macOS defaults
    if let Some(home_dir) = home_dir() {
        candidates.push(home_dir.join("Library").join("Huawei").join("Sdk"));
        candidates.push(home_dir.join("Library").join("OpenHarmony").join("Sdk"));
    }

    // Windows default
    if let Ok(app_data) = std::env::var("APPDATA") {
        // APPDATA points to Roaming; Local is one level up + Local
        let roaming = PathBuf::from(&app_data);
        if let Some(local) = roaming
            .parent()
            .map(|p| p.join("Local").join("Huawei").join("Sdk"))
        {
            candidates.push(local);
        }
    }

    // Linux default
    candidates.push(PathBuf::from("/opt/huawei/sdk"));

    candidates
}

/// Return the platform string used in es2abc path (`build-linux`, `build-mac`, `build-win`).
fn platform_dir() -> &'static str {
    if cfg!(target_os = "macos") {
        "build-mac"
    } else if cfg!(target_os = "windows") {
        "build-win"
    } else {
        "build-linux"
    }
}

struct SdkToolResolver<'a> {
    ets_loader_path: &'a Path,
}

impl<'a> SdkToolResolver<'a> {
    fn new(ets_loader_path: &'a Path) -> Self {
        Self { ets_loader_path }
    }

    fn es2abc_path(&self) -> PathBuf {
        self.existing_es2abc_path()
            .unwrap_or_else(|| self.platform_es2abc_path())
    }

    fn existing_es2abc_path(&self) -> Option<PathBuf> {
        self.es2abc_candidates()
            .into_iter()
            .find(|path| path.is_file())
    }

    fn es2abc_candidates(&self) -> [PathBuf; 2] {
        let ark_dir = self.ets_loader_path.join("bin").join("ark");
        [
            self.platform_es2abc_path(),
            ark_dir.join("build").join("bin").join("es2abc"),
        ]
    }

    fn platform_es2abc_path(&self) -> PathBuf {
        self.ets_loader_path
            .join("bin")
            .join("ark")
            .join(platform_dir())
            .join("bin")
            .join("es2abc")
    }
}

/// Attempt to discover the OHOS SDK root directory.
///
/// Iterates the candidate list and returns the first directory that exists.
pub fn find_sdk_root() -> Option<PathBuf> {
    sdk_candidates()
        .into_iter()
        .find(|candidate| candidate.exists())
}

/// Build [`SdkConfig`] from a known SDK root path.
///
/// `target_api_version` comes from an explicit `targetSdkVersion`. When it is
/// absent, hvigor uses the selected SDK component `apiVersion`.
pub fn sdk_config_from_root(sdk_root: &Path, target_api_version: Option<u32>) -> SdkConfig {
    sdk_config_from_root_with_selection(
        sdk_root,
        &SdkSelection {
            target_api_version,
            compile_sdk_version: None,
        },
    )
}

fn sdk_config_from_root_with_selection(sdk_root: &Path, selection: &SdkSelection) -> SdkConfig {
    let ohos_base = openharmony_base_from_root(sdk_root);
    let sdk_metadata = discover_sdk_metadata(&ohos_base);
    let compile_api_version = selection
        .target_api_version
        .or(sdk_metadata.api_version)
        .unwrap_or(12);

    let ets_loader_path = ohos_base.join("ets").join("build-tools").join("ets-loader");
    let es2abc_path = SdkToolResolver::new(&ets_loader_path).es2abc_path();
    let restool_path = ohos_base.join("toolchains").join("restool");

    let has_rollup_plugin = compile_api_version >= 9;

    SdkConfig {
        es2abc_path,
        restool_path,
        ets_loader_path,
        compile_api_version,
        has_rollup_plugin,
        compile_sdk_version: selection
            .compile_sdk_version
            .clone()
            .or(sdk_metadata.compile_sdk_version)
            .unwrap_or_else(default_compile_sdk_version),
    }
}

/// Discover the SDK and build an [`SdkConfig`].
///
/// `target_api_version` is derived from explicit `targetSdkVersion`; when
/// missing, the SDK component metadata decides the API.
/// If the SDK root is not found, returns [`ConfigError::SdkNotFound`].
pub fn discover_sdk(target_api_version: Option<u32>) -> Result<SdkConfig, ConfigError> {
    discover_sdk_with_selection(SdkSelection {
        target_api_version,
        compile_sdk_version: None,
    })
}

/// Discover the configured SDK and apply metadata overrides.
///
/// Hvigor cache values influence generated profile metadata and target API,
/// not the SDK component path. Tool paths continue to come from normal SDK
/// discovery (`DEVECO_SDK_HOME`, `OHOS_BASE_SDK_HOME`, `OHOS_SDK_HOME`, etc.).
pub fn discover_sdk_with_selection(selection: SdkSelection) -> Result<SdkConfig, ConfigError> {
    let sdk_root = find_sdk_root().ok_or(ConfigError::SdkNotFound)?;
    Ok(sdk_config_from_root_with_selection(&sdk_root, &selection))
}

/// Discover `app.compileSdkVersion` from SDK component metadata.
pub fn discover_compile_sdk_version(ohos_base: &Path) -> Option<String> {
    discover_sdk_metadata(ohos_base).compile_sdk_version
}

/// Discover the selected SDK `apiVersion`.
pub fn discover_sdk_api_version(ohos_base: &Path) -> Option<u32> {
    discover_sdk_metadata(ohos_base).api_version
}

/// Discover the raw Ark VM version from the same SDK tooling hvigor uses.
///
/// hvigor calls `ets-loader/bin/ark/ts2abc.js --target-api-version <api>` and,
/// when the SDK exposes a compatible API stage, passes
/// `--target-api-sub-version <stage>`. The stage is read from the SDK
/// `oh-uni-package.json` metadata here, so the local SDK decides the exact
/// mapping.
pub fn discover_ark_vm_version(ets_loader_path: &Path, compile_api_version: u32) -> Option<String> {
    let stage = discover_sdk_release_type_from_ets_loader(ets_loader_path);
    discover_ark_vm_version_with_stage(ets_loader_path, compile_api_version, stage.as_deref())
}

fn discover_ark_vm_version_with_stage(
    ets_loader_path: &Path,
    compile_api_version: u32,
    stage: Option<&str>,
) -> Option<String> {
    discover_ark_vm_version_from_ts2abc(ets_loader_path, compile_api_version, stage).or_else(|| {
        discover_ark_vm_version_from_es2abc(ets_loader_path, compile_api_version, stage)
    })
}

fn openharmony_base_from_root(sdk_root: &Path) -> PathBuf {
    let default_openharmony = sdk_root.join("default").join("openharmony");
    if looks_like_openharmony_base(&default_openharmony) {
        return default_openharmony;
    }

    if looks_like_openharmony_base(sdk_root) {
        return sdk_root.to_path_buf();
    }

    let openharmony = sdk_root.join("openharmony");
    if looks_like_openharmony_base(&openharmony) {
        return openharmony;
    }

    default_openharmony
}

#[derive(Debug, Default)]
struct SdkMetadata {
    api_version: Option<u32>,
    compile_sdk_version: Option<String>,
}

fn discover_sdk_metadata(ohos_base: &Path) -> SdkMetadata {
    let mut metadata = SdkMetadata::default();
    for path in sdk_metadata_paths(ohos_base) {
        let Some(value) = read_json_value(&path) else {
            continue;
        };
        if metadata.compile_sdk_version.is_none() {
            metadata.compile_sdk_version = json_string_field(&value, "version");
        }
        if metadata.api_version.is_none() {
            metadata.api_version = json_u32_field(&value, "apiVersion");
        }
        if metadata.compile_sdk_version.is_some() && metadata.api_version.is_some() {
            break;
        }
    }
    metadata
}

fn sdk_metadata_paths(ohos_base: &Path) -> [PathBuf; 5] {
    [
        ohos_base.join("ets").join("oh-uni-package.json"),
        ohos_base.join("native").join("oh-uni-package.json"),
        ohos_base.join("toolchains").join("oh-uni-package.json"),
        ohos_base.join("js").join("oh-uni-package.json"),
        ohos_base.join("previewer").join("oh-uni-package.json"),
    ]
}

fn looks_like_openharmony_base(path: &Path) -> bool {
    path.join("ets").exists() || path.join("native").exists() || path.join("toolchains").exists()
}

fn default_compile_sdk_version() -> String {
    "6.0.0.47".to_string()
}

fn discover_sdk_release_type_from_ets_loader(ets_loader_path: &Path) -> Option<String> {
    let ets_component = ets_loader_path.parent()?.parent()?;
    read_json_string_field(&ets_component.join("oh-uni-package.json"), "releaseType")
}

fn discover_ark_vm_version_from_ts2abc(
    ets_loader_path: &Path,
    compile_api_version: u32,
    stage: Option<&str>,
) -> Option<String> {
    let ts2abc_path = ets_loader_path.join("bin").join("ark").join("ts2abc.js");
    if !ts2abc_path.exists() {
        return None;
    }

    let mut command = Command::new("node");
    command
        .arg(ts2abc_path)
        .arg("--target-api-version")
        .arg(compile_api_version.to_string());
    if let Some(stage) = non_empty_stage(stage) {
        command.arg("--target-api-sub-version").arg(stage);
    }
    run_ark_version_command(command)
}

fn discover_ark_vm_version_from_es2abc(
    ets_loader_path: &Path,
    compile_api_version: u32,
    stage: Option<&str>,
) -> Option<String> {
    let es2abc_path = SdkToolResolver::new(ets_loader_path).existing_es2abc_path()?;
    let mut command = Command::new(es2abc_path);
    command
        .arg("--target-bc-version")
        .arg("--target-api-version")
        .arg(compile_api_version.to_string());
    if let Some(stage) = non_empty_stage(stage) {
        command.arg("--target-api-sub-version").arg(stage);
    }
    run_ark_version_command(command)
}

fn non_empty_stage(stage: Option<&str>) -> Option<&str> {
    stage.map(str::trim).filter(|value| !value.is_empty())
}

fn run_ark_version_command(mut command: Command) -> Option<String> {
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_ark_vm_command_output(&output.stdout)
}

fn parse_ark_vm_command_output(output: &[u8]) -> Option<String> {
    let stdout = String::from_utf8_lossy(output);
    stdout
        .split_whitespace()
        .find_map(|raw| ArkVmVersionText::new(raw).normalized())
}

struct ArkVmVersionText<'a> {
    raw: &'a str,
}

impl<'a> ArkVmVersionText<'a> {
    const PREFIX: &'static str = "ark";

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn normalized(&self) -> Option<String> {
        let version = self
            .raw
            .trim()
            .strip_prefix(Self::PREFIX)
            .unwrap_or(self.raw.trim());
        Self::is_dotted_numeric(version).then(|| version.to_string())
    }

    fn is_dotted_numeric(version: &str) -> bool {
        let mut part_count = 0;
        for part in version.split('.') {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return false;
            }
            part_count += 1;
        }
        part_count >= 2
    }
}

fn read_json_string_field(path: &Path, field: &str) -> Option<String> {
    let value = read_json_value(path)?;
    json_string_field(&value, field)
}

fn read_json_value(path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn json_field<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a serde_json::Value> {
    value
        .get(field)
        .or_else(|| value.get("data").and_then(|data| data.get(field)))
}

fn json_string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    let field_value = json_field(value, field)?;
    field_value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn json_u32_field(value: &serde_json::Value, field: &str) -> Option<u32> {
    let field_value = json_field(value, field)?;
    if let Some(number) = field_value.as_u64() {
        return u32::try_from(number).ok();
    }
    field_value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse().ok())
}

// ---------------------------------------------------------------------------
// Helper — portable home directory
// ---------------------------------------------------------------------------

fn home_dir() -> Option<PathBuf> {
    // std::env::home_dir is deprecated but still works; use HOME env var for reliability.
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home));
    }
    // Windows USERPROFILE
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return Some(PathBuf::from(profile));
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("arkdown-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn test_sdk_discovery_no_panic() {
        // When SDK is not installed, discover_sdk must return SdkNotFound, never panic.
        // Override OHOS_SDK_HOME to a nonexistent path to ensure controlled behaviour.
        // (We cannot unset the env var in a portable way inside a test, so we just call
        // discover_sdk and accept either Ok or SdkNotFound.)
        let result = discover_sdk(Some(12));
        match result {
            Ok(cfg) => {
                // If an SDK root was found, verify version propagation.
                assert_eq!(cfg.compile_api_version, 12);
                assert!(cfg.has_rollup_plugin); // 12 >= 9
            }
            Err(ConfigError::SdkNotFound) => {
                // Expected on CI / machines without DevEco Studio
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn test_sdk_config_from_root_paths() {
        let root = PathBuf::from("/fake/sdk");
        let cfg = sdk_config_from_root(&root, Some(12));
        assert_eq!(cfg.compile_api_version, 12);
        assert!(cfg.has_rollup_plugin);
        assert_eq!(cfg.compile_sdk_version, "6.0.0.47");
        // es2abc must be nested inside the ets-loader/bin/ark/{platform}/bin/ tree.
        assert!(cfg.es2abc_path.to_string_lossy().contains("ets-loader"));
        assert!(cfg.restool_path.to_string_lossy().contains("restool"));
        assert!(cfg.ets_loader_path.to_string_lossy().contains("ets-loader"));
        assert!(cfg.pac_schema_path().is_none());
    }

    #[test]
    fn test_sdk_config_from_root_accepts_linux_sdk_generic_ark_build_layout() {
        let root = temp_root("linux-sdk-es2abc");
        let ets_loader = root.join("ets").join("build-tools").join("ets-loader");
        let es2abc = ets_loader
            .join("bin")
            .join("ark")
            .join("build")
            .join("bin")
            .join("es2abc");
        fs::create_dir_all(es2abc.parent().expect("es2abc parent"))
            .expect("create generic ark build dir");
        fs::write(&es2abc, []).expect("write fake es2abc");

        // setup-ohos-sdk's Linux SDK can expose host tools under ark/build
        // instead of the platform-qualified ark/build-linux directory.
        let cfg = sdk_config_from_root(&root, Some(12));

        assert_eq!(cfg.es2abc_path, es2abc);

        let platform_es2abc = SdkToolResolver::new(&ets_loader).platform_es2abc_path();
        fs::create_dir_all(platform_es2abc.parent().expect("platform es2abc parent"))
            .expect("create platform ark build dir");
        fs::write(&platform_es2abc, []).expect("write platform es2abc");

        let cfg = sdk_config_from_root(&root, Some(12));

        assert_eq!(cfg.es2abc_path, platform_es2abc);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_sdk_config_from_root_reads_compile_sdk_version() {
        let root = temp_root("sdk-version");
        let ets_dir = root.join("default").join("openharmony").join("ets");
        fs::create_dir_all(&ets_dir).expect("create sdk metadata dir");
        fs::write(
            ets_dir.join("oh-uni-package.json"),
            r#"{"version":"6.0.1.112","releaseType":"Release"}"#,
        )
        .expect("write sdk metadata");

        let cfg = sdk_config_from_root(&root, Some(12));

        assert_eq!(cfg.compile_sdk_version, "6.0.1.112");
        assert!(cfg
            .ets_loader_path
            .starts_with(root.join("default").join("openharmony")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_sdk_config_from_root_uses_sdk_api_when_target_absent() {
        let root = temp_root("sdk-api-version");
        let ets_dir = root.join("default").join("openharmony").join("ets");
        fs::create_dir_all(&ets_dir).expect("create sdk metadata dir");
        fs::write(
            ets_dir.join("oh-uni-package.json"),
            r#"{"apiVersion":"21","version":"6.0.1.112","releaseType":"Release"}"#,
        )
        .expect("write sdk metadata");

        let cfg = sdk_config_from_root(&root, None);

        assert_eq!(cfg.compile_api_version, 21);
        assert_eq!(
            discover_sdk_api_version(&root.join("default/openharmony")),
            Some(21)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_sdk_config_from_root_applies_hvigor_metadata_without_switching_component() {
        let root = temp_root("sdk-metadata-overlay");
        let default_ets = root.join("default").join("openharmony").join("ets");
        let api20_ets = root.join("20").join("ets");
        fs::create_dir_all(&default_ets).expect("create default sdk metadata dir");
        fs::create_dir_all(&api20_ets).expect("create api20 sdk metadata dir");
        fs::write(
            default_ets.join("oh-uni-package.json"),
            r#"{"apiVersion":"21","version":"6.0.1.112","releaseType":"Release"}"#,
        )
        .expect("write default metadata");
        fs::write(
            api20_ets.join("oh-uni-package.json"),
            r#"{"apiVersion":"20","version":"6.0.0.47","releaseType":"Release"}"#,
        )
        .expect("write api20 metadata");

        let cfg = sdk_config_from_root_with_selection(
            &root,
            &SdkSelection {
                target_api_version: Some(20),
                compile_sdk_version: Some("6.0.0.47".to_string()),
            },
        );

        assert_eq!(cfg.compile_api_version, 20);
        assert_eq!(cfg.compile_sdk_version, "6.0.0.47");
        assert!(cfg
            .ets_loader_path
            .starts_with(root.join("default").join("openharmony")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_sdk_config_discovers_pac_schema_when_present() {
        let root = temp_root("pac-schema");
        let ets_dir = root.join("default").join("openharmony").join("ets");
        let schema = root
            .join("default")
            .join("openharmony")
            .join("toolchains/paccheck/pacSchema.json");
        fs::create_dir_all(&ets_dir).expect("create sdk metadata dir");
        fs::create_dir_all(schema.parent().unwrap()).expect("create pac schema dir");
        fs::write(&schema, "{}").expect("write pac schema");

        let cfg = sdk_config_from_root(&root, Some(12));

        assert_eq!(cfg.pac_schema_path().as_deref(), Some(schema.as_path()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_sdk_config_discovers_permission_module_when_present() {
        let root = temp_root("permission-module");
        let ets_dir = root.join("default").join("openharmony").join("ets");
        let permission_module = root
            .join("default")
            .join("openharmony")
            .join("previewer/common/resources/module.json");
        fs::create_dir_all(&ets_dir).expect("create sdk metadata dir");
        fs::create_dir_all(permission_module.parent().unwrap())
            .expect("create permission module dir");
        fs::write(&permission_module, "{}").expect("write permission module");

        let cfg = sdk_config_from_root(&root, Some(12));

        assert_eq!(
            cfg.permission_module_json_path().as_deref(),
            Some(permission_module.as_path())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_sdk_config_accepts_direct_openharmony_root() {
        let root = temp_root("direct-sdk");
        let ets_dir = root.join("ets");
        fs::create_dir_all(&ets_dir).expect("create direct sdk metadata dir");
        fs::write(
            ets_dir.join("oh-uni-package.json"),
            r#"{"version":"6.1.0.31","releaseType":"Release"}"#,
        )
        .expect("write direct sdk metadata");

        let cfg = sdk_config_from_root(&root, Some(17));

        assert_eq!(cfg.compile_sdk_version, "6.1.0.31");
        assert!(cfg.ets_loader_path.starts_with(&root));
        assert!(!cfg
            .ets_loader_path
            .starts_with(root.join("default").join("openharmony")));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn test_discover_ark_vm_version_uses_sdk_release_type() {
        let root = temp_root("ark-vm");
        let ets_dir = root.join("ets");
        let ets_loader = ets_dir.join("build-tools").join("ets-loader");
        let es2abc = ets_loader
            .join("bin")
            .join("ark")
            .join(platform_dir())
            .join("bin")
            .join("es2abc");

        fs::create_dir_all(&ets_dir).expect("create ets dir");
        fs::create_dir_all(es2abc.parent().expect("es2abc parent")).expect("create es2abc dir");
        fs::write(
            ets_dir.join("oh-uni-package.json"),
            r#"{"version":"6.0.1.112","releaseType":"Release"}"#,
        )
        .expect("write ets metadata");
        fs::write(
            &es2abc,
            r#"#!/bin/sh
stage=""
previous=""
for arg in "$@"; do
  if [ "$previous" = "--target-api-sub-version" ]; then
    stage="$arg"
  fi
  previous="$arg"
done
if [ "$stage" = "Release" ]; then
  printf '12.0.6.0\n'
else
  printf '12.0.2.0\n'
fi
"#,
        )
        .expect("write fake es2abc");
        let mut permissions = fs::metadata(&es2abc)
            .expect("fake es2abc metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&es2abc, permissions).expect("chmod fake es2abc");

        let ark_vm_version = discover_ark_vm_version(&ets_loader, 12);

        assert_eq!(ark_vm_version.as_deref(), Some("12.0.6.0"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_has_rollup_plugin_threshold() {
        let root = PathBuf::from("/fake/sdk");
        let cfg_v9 = sdk_config_from_root(&root, Some(9));
        assert!(cfg_v9.has_rollup_plugin);
        let cfg_v8 = sdk_config_from_root(&root, Some(8));
        assert!(!cfg_v8.has_rollup_plugin);
    }
}
