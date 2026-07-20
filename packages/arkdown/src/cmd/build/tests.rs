use tempfile::TempDir;

use super::layout::RestoolLayout;

#[test]
fn restool_layout_matches_hvigor_sdk_specific_image_transcoder_contract() {
  let temp = TempDir::new().expect("create SDK fixture root");

  let openharmony_toolchains = temp.path().join("openharmony-sdk").join("toolchains");
  std::fs::create_dir_all(&openharmony_toolchains).expect("create OpenHarmony toolchains");
  std::fs::write(openharmony_toolchains.join("oh-uni-package.json"), b"{}")
    .expect("write OpenHarmony component marker");
  let openharmony_layout =
    RestoolLayout::from_restool_path(&openharmony_toolchains.join("restool"));
  assert_eq!(
    openharmony_layout.image_transcoder_library(),
    None,
    "hvigor OhosSdkInfo.getLibimageTranscoderShared() returns an empty path"
  );
  assert_eq!(
    openharmony_layout.defined_sys_ids(),
    None,
    "hvigor ProcessResource omits definedSysIds for OpenHarmony SDKs"
  );

  let default_sdk = temp.path().join("deveco-sdk").join("default");
  let openharmony_restool = default_sdk
    .join("openharmony")
    .join("toolchains")
    .join("restool");
  let hms_toolchains = default_sdk.join("hms").join("toolchains");
  std::fs::create_dir_all(&hms_toolchains).expect("create HarmonyOS toolchains");
  std::fs::write(hms_toolchains.join("id_defined.json"), b"[]")
    .expect("write HarmonyOS system ID definitions");
  let harmony_layout = RestoolLayout::from_restool_path(&openharmony_restool);
  let library = harmony_layout
    .image_transcoder_library()
    .expect("HarmonyOS SDK must expose the image transcoder path");
  assert_eq!(library.parent(), Some(hms_toolchains.join("lib").as_path()));
  assert_eq!(
    library.file_name().and_then(|name| name.to_str()),
    Some(platform_image_transcoder_name())
  );
  assert_eq!(
    harmony_layout.defined_sys_ids(),
    Some(hms_toolchains.join("id_defined.json"))
  );
}

fn platform_image_transcoder_name() -> &'static str {
  if cfg!(target_os = "windows") {
    "libimage_transcoder_shared.dll"
  } else if cfg!(target_os = "linux") {
    "libimage_transcoder_shared.so"
  } else {
    "libimage_transcoder_shared.dylib"
  }
}
