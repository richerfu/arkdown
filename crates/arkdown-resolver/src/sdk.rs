use std::path::{Path, PathBuf};

/// Returns the path to the SDK `api/` directory for ETS APIs (`@ohos/*`).
///
/// Typical layout: `<sdk_root>/default/openharmony/ets/api`
pub fn sdk_api_dir(sdk_root: &Path) -> PathBuf {
    sdk_root.join("default/openharmony/ets/api")
}

/// Returns the path to the SDK `kits/` directory (`@kit.*`).
///
/// Typical layout: `<sdk_root>/default/openharmony/ets/kits`
pub fn sdk_kits_dir(sdk_root: &Path) -> PathBuf {
    sdk_root.join("default/openharmony/ets/kits")
}
