use std::path::{Path, PathBuf};

use napi::Env;

/// Owns discovery of native services shipped by arkdown's npm packages.
///
/// The top-level JavaScript package can load this binding either from a local
/// napi-rs workspace layout or from an installed optional platform package.
/// Keeping both layouts here leaves JavaScript responsible only for the public
/// CLI and plugin ABI, not compiler-service discovery.
pub(crate) struct NapiPackageLayout {
  native_module: PathBuf,
}

impl NapiPackageLayout {
  pub(crate) fn from_env(env: &Env) -> napi::Result<Self> {
    let module_url = env.get_module_file_name()?;
    let native_module = url::Url::parse(&module_url)
      .map_err(|error| {
        napi::Error::from_reason(format!(
          "ARKDOWN_NAPI_MODULE_URL_INVALID: {module_url}: {error}"
        ))
      })?
      .to_file_path()
      .map_err(|()| {
        napi::Error::from_reason(format!("ARKDOWN_NAPI_MODULE_URL_NOT_FILE: {module_url}"))
      })?;
    Ok(Self { native_module })
  }

  /// Returns only an executable present in a supported package layout.
  /// `None` is intentional: it lets `NativeTypeSession` apply the explicit
  /// development override before returning its typed missing-service error.
  pub(crate) fn packaged_tsrs_executable(&self) -> napi::Result<Option<PathBuf>> {
    let package_directory = self.package_directory()?;
    let executable_name = arkdown_path::HostPlatform::CURRENT.executable_name("tsrs");
    let direct = package_directory.join(&executable_name);
    if direct.is_file() {
      return Ok(Some(direct));
    }

    let Some(platform_directory) = self.platform_package_directory() else {
      return Ok(None);
    };
    let local_platform = package_directory
      .join("npm")
      .join(platform_directory)
      .join(&executable_name);
    if local_platform.is_file() {
      return Ok(Some(local_platform));
    }

    let installed_platform = package_directory
      .parent()
      .map(|scope_directory| {
        scope_directory
          .join(format!("arkdown-{platform_directory}"))
          .join(&executable_name)
      })
      .filter(|candidate| candidate.is_file());
    Ok(installed_platform)
  }

  fn package_directory(&self) -> napi::Result<&Path> {
    self.native_module.parent().ok_or_else(|| {
      napi::Error::from_reason(format!(
        "ARKDOWN_NAPI_PACKAGE_LAYOUT_INVALID: {} has no parent directory",
        self.native_module.display()
      ))
    })
  }

  fn platform_package_directory(&self) -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
      ("macos", "aarch64") => Some("darwin-arm64"),
      ("linux", "x86_64") => Some("linux-x64-gnu"),
      ("windows", "x86_64") => Some("win32-x64-msvc"),
      _ => None,
    }
  }
}

#[cfg(test)]
#[path = "package_layout_tests.rs"]
mod package_layout_tests;
