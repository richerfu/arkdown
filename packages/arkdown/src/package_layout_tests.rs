use std::fs;

use super::NapiPackageLayout;

#[test]
fn resolves_tsrs_from_local_napi_platform_package() {
  let workspace = tempfile::tempdir().expect("temporary package workspace");
  let package_directory = workspace.path().join("arkdown");
  let native_module = package_directory.join("index.js");
  fs::create_dir_all(&package_directory).expect("top-level package directory");
  fs::write(&native_module, "module.exports = {}\n").expect("top-level package entry");

  let layout = NapiPackageLayout { native_module };
  let platform_directory = layout
    .platform_package_directory()
    .expect("test host must be an npm-supported arkdown target");
  let executable = package_directory
    .join("npm")
    .join(platform_directory)
    .join(arkdown_path::HostPlatform::CURRENT.executable_name("tsrs"));
  fs::create_dir_all(executable.parent().expect("platform package directory"))
    .expect("platform package directory");
  fs::write(&executable, b"native tsrs service").expect("tsrs executable");

  assert_eq!(
    layout
      .packaged_tsrs_executable()
      .expect("valid package layout"),
    Some(executable)
  );
}

#[test]
fn resolves_tsrs_from_installed_optional_platform_package() {
  let workspace = tempfile::tempdir().expect("temporary node_modules workspace");
  let scope_directory = workspace.path().join("node_modules/@ohos-rs");
  let package_directory = scope_directory.join("arkdown");
  let native_module = package_directory.join("index.js");
  fs::create_dir_all(&package_directory).expect("top-level package directory");
  fs::write(&native_module, "module.exports = {}\n").expect("top-level package entry");

  let layout = NapiPackageLayout { native_module };
  let platform_directory = layout
    .platform_package_directory()
    .expect("test host must be an npm-supported arkdown target");
  let executable = scope_directory
    .join(format!("arkdown-{platform_directory}"))
    .join(arkdown_path::HostPlatform::CURRENT.executable_name("tsrs"));
  fs::create_dir_all(executable.parent().expect("platform package directory"))
    .expect("platform package directory");
  fs::write(&executable, b"native tsrs service").expect("tsrs executable");

  assert_eq!(
    layout
      .packaged_tsrs_executable()
      .expect("valid package layout"),
    Some(executable)
  );
}

#[test]
fn missing_packaged_tsrs_remains_available_for_explicit_override() {
  let workspace = tempfile::tempdir().expect("temporary package workspace");
  let package_directory = workspace.path().join("arkdown");
  fs::create_dir_all(&package_directory).expect("top-level package directory");
  let layout = NapiPackageLayout {
    native_module: package_directory.join("index.js"),
  };

  assert_eq!(
    layout
      .packaged_tsrs_executable()
      .expect("valid package layout"),
    None
  );
}
