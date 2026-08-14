use std::path::PathBuf;

use arkdown_session::{HvigorSdkDetailsError, HvigorSdkDetailsInventory, HvigorSdkDetailsRequest};

#[test]
fn sdk_details_select_requested_component_from_ets_loader_root() {
    let root = tempfile::tempdir().expect("temp sdk");
    let sdk_root = root.path().join("sdk");
    let loader = sdk_root.join("12").join("ets").join("build-tools");
    std::fs::create_dir_all(&loader).expect("loader root");
    std::fs::write(
        sdk_root.join("12").join("ets").join("oh-uni-package.json"),
        r#"{"data":{"apiVersion":"6.1.1(12)","version":"6.1.1.280","releaseType":"Release"}}"#,
    )
    .expect("metadata");

    let snapshot = HvigorSdkDetailsInventory::new(HvigorSdkDetailsRequest {
        project_root: root.path().to_path_buf(),
        sdk_path: None,
        ets_loader_path: Some(loader),
        is_ohos: false,
        requested_api: 12,
    })
    .inspect()
    .expect("SDK details");

    assert_eq!(snapshot.sdk_dir, sdk_root.join("12"));
    let metadata = snapshot.metadata.expect("selected metadata");
    assert_eq!(metadata.api_version, Some(12));
    assert_eq!(metadata.version.as_deref(), Some("6.1.1.280"));
    assert_eq!(metadata.release_type.as_deref(), Some("Release"));
}

#[test]
fn open_harmony_sdk_details_read_escaped_local_properties_path() {
    let root = tempfile::tempdir().expect("temp project");
    let sdk_root = root.path().join("OpenHarmony SDK");
    std::fs::create_dir_all(&sdk_root).expect("SDK root");
    let escaped = sdk_root.to_string_lossy().replace(':', "\\:");
    std::fs::write(
        root.path().join("local.properties"),
        format!("other=value\nsdk.dir={escaped}\n"),
    )
    .expect("local.properties");

    let snapshot = HvigorSdkDetailsInventory::new(HvigorSdkDetailsRequest {
        project_root: root.path().to_path_buf(),
        sdk_path: None,
        ets_loader_path: None,
        is_ohos: true,
        requested_api: 10,
    })
    .inspect()
    .expect("SDK details");

    assert_eq!(snapshot.sdk_dir, PathBuf::from(sdk_root));
}

#[test]
fn invalid_component_metadata_is_not_hidden_by_a_fallback() {
    let root = tempfile::tempdir().expect("temp SDK");
    let metadata = root.path().join("ets").join("oh-uni-package.json");
    std::fs::create_dir_all(metadata.parent().expect("metadata parent")).expect("metadata root");
    std::fs::write(&metadata, "not-json").expect("invalid metadata");

    let error = HvigorSdkDetailsInventory::new(HvigorSdkDetailsRequest {
        project_root: root.path().to_path_buf(),
        sdk_path: Some(root.path().to_path_buf()),
        ets_loader_path: None,
        is_ohos: false,
        requested_api: 10,
    })
    .inspect()
    .expect_err("invalid metadata must fail");

    assert!(matches!(
        error,
        HvigorSdkDetailsError::MetadataParse { path, .. } if path == metadata
    ));
}
