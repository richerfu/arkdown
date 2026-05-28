use std::path::{Path, PathBuf};

use arkdown_core::SdkConfig;

use crate::error::ConfigError;

// ---------------------------------------------------------------------------
// SDK path discovery
// ---------------------------------------------------------------------------

/// Return candidate SDK root paths to try in priority order.
///
/// Order:
/// 1. `OHOS_SDK_HOME` environment variable
/// 2. `~/Library/Huawei/Sdk/` (macOS)
/// 3. `~/AppData/Local/Huawei/Sdk/` (Windows)
/// 4. `/opt/huawei/sdk/` (Linux)
fn sdk_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. Env var
    if let Ok(home) = std::env::var("OHOS_SDK_HOME") {
        candidates.push(PathBuf::from(home));
    }

    // 2. macOS default
    if let Some(home_dir) = home_dir() {
        candidates.push(home_dir.join("Library").join("Huawei").join("Sdk"));
    }

    // 3. Windows default
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

    // 4. Linux default
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

/// Attempt to discover the OHOS SDK root directory.
///
/// Iterates the candidate list and returns the first directory that exists.
pub fn find_sdk_root() -> Option<PathBuf> {
    for candidate in sdk_candidates() {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Build [`SdkConfig`] from a known SDK root path and a numeric API version.
pub fn sdk_config_from_root(sdk_root: &Path, compile_api_version: u32) -> SdkConfig {
    let ohos_base = sdk_root.join("default").join("openharmony");
    let platform = platform_dir();

    let es2abc_path = ohos_base
        .join("ets")
        .join("build-tools")
        .join("ets-loader")
        .join("bin")
        .join("ark")
        .join(format!("{platform}"))
        .join("bin")
        .join("es2abc");

    let restool_path = ohos_base.join("toolchains").join("restool");

    let ets_loader_path = ohos_base
        .join("ets")
        .join("build-tools")
        .join("ets-loader");

    let has_rollup_plugin = compile_api_version >= 9;

    SdkConfig {
        es2abc_path,
        restool_path,
        ets_loader_path,
        compile_api_version,
        has_rollup_plugin,
        compile_sdk_version: "6.0.0.47".to_string(),
    }
}

/// Discover the SDK and build an [`SdkConfig`].
///
/// `compile_api_version` is derived from `compatibleSdkVersion` in the build profile.
/// If the SDK root is not found, returns [`ConfigError::SdkNotFound`].
pub fn discover_sdk(compile_api_version: u32) -> Result<SdkConfig, ConfigError> {
    let sdk_root = find_sdk_root().ok_or(ConfigError::SdkNotFound)?;
    Ok(sdk_config_from_root(&sdk_root, compile_api_version))
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

    #[test]
    fn test_sdk_discovery_no_panic() {
        // When SDK is not installed, discover_sdk must return SdkNotFound, never panic.
        // Override OHOS_SDK_HOME to a nonexistent path to ensure controlled behaviour.
        // (We cannot unset the env var in a portable way inside a test, so we just call
        // discover_sdk and accept either Ok or SdkNotFound.)
        let result = discover_sdk(12);
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
        let cfg = sdk_config_from_root(&root, 12);
        assert_eq!(cfg.compile_api_version, 12);
        assert!(cfg.has_rollup_plugin);
        // es2abc must be nested inside the ets-loader/bin/ark/{platform}/bin/ tree.
        assert!(cfg
            .es2abc_path
            .to_string_lossy()
            .contains("ets-loader"));
        assert!(cfg
            .restool_path
            .to_string_lossy()
            .contains("restool"));
        assert!(cfg
            .ets_loader_path
            .to_string_lossy()
            .contains("ets-loader"));
    }

    #[test]
    fn test_has_rollup_plugin_threshold() {
        let root = PathBuf::from("/fake/sdk");
        let cfg_v9 = sdk_config_from_root(&root, 9);
        assert!(cfg_v9.has_rollup_plugin);
        let cfg_v8 = sdk_config_from_root(&root, 8);
        assert!(!cfg_v8.has_rollup_plugin);
    }
}
