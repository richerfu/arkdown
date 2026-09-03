use std::str::FromStr;

use std::path::PathBuf;

use arkdown_core::{ArktsLanguageVersion, SdkConfig};

#[test]
fn arkts_1_2_requires_the_explicit_public_value() {
    assert_eq!(ArktsLanguageVersion::default(), ArktsLanguageVersion::V1_1);
    assert_eq!(
        ArktsLanguageVersion::from_str("1.2").expect("explicit ArkTS 1.2"),
        ArktsLanguageVersion::V1_2
    );
    assert!(ArktsLanguageVersion::from_str("static").is_err());
}

#[test]
fn static_language_uses_the_sdk_static_toolchain_layout() {
    let config = SdkConfig {
        ets_loader_path: PathBuf::from("/sdk/ets/build-tools/ets-loader"),
        es2abc_path: PathBuf::from("/sdk/ets/build-tools/ets-loader/bin/ark/es2abc"),
        restool_path: PathBuf::from("/sdk/toolchains/restool"),
        compile_api_version: 20,
        sdk_component_api_version: 20,
        has_rollup_plugin: true,
        compile_sdk_version: "7.0.0.1".to_string(),
    };
    let layout = config.static_arkts_layout().expect("static SDK layout");

    assert_eq!(layout.root, PathBuf::from("/sdk/ets/static"));
    assert_eq!(
        layout.driver_entry,
        PathBuf::from("/sdk/ets/static/build-tools/driver/build-system/dist/entry.js")
    );
    assert_eq!(
        layout.panda_sdk,
        PathBuf::from("/sdk/ets/static/build-tools/ets2panda")
    );
    assert_eq!(
        layout.ui_plugin,
        PathBuf::from("/sdk/ets/static/build-tools/ui-plugins/lib/ui-plugins/index")
    );
    assert_eq!(layout.api, PathBuf::from("/sdk/ets/static/api"));
    assert_eq!(layout.kits, PathBuf::from("/sdk/ets/static/kits"));
    assert_eq!(layout.arkts, PathBuf::from("/sdk/ets/static/arkts"));
}
