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
fn merged_bytecode_name_follows_the_selected_frontend() {
    assert_eq!(
        ArktsLanguageVersion::V1_1.merged_abc_file_name(),
        "modules.abc"
    );
    assert_eq!(
        ArktsLanguageVersion::V1_2.merged_abc_file_name(),
        "modules_static.abc"
    );
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
        layout.libarkts_entry,
        PathBuf::from("/sdk/ets/static/build-tools/libarkts/lib/libarkts.js")
    );
    assert_eq!(
        layout.es2panda_addon_candidates,
        [
            PathBuf::from("/sdk/ets/static/build-tools/libarkts/native/build/es2panda.node"),
            PathBuf::from("/sdk/ets/static/build-tools/libarkts/build/native/es2panda.node"),
            PathBuf::from("/sdk/ets/static/build-tools/libarkts/build/native/build/es2panda.node"),
        ]
    );
    assert_eq!(
        layout.etsstdlib_abc,
        PathBuf::from("/sdk/ets/static/build-tools/ets2panda/lib/etsstdlib.abc")
    );
    assert_eq!(
        layout.stdlib,
        PathBuf::from("/sdk/ets/static/build-tools/ets2panda/lib/stdlib")
    );
    assert_eq!(
        layout.ui_plugin,
        PathBuf::from("/sdk/ets/static/build-tools/ui-plugins/lib/ui-plugins/index")
    );
    assert_eq!(layout.api, PathBuf::from("/sdk/ets/static/api"));
    assert_eq!(layout.kits, PathBuf::from("/sdk/ets/static/kits"));
    assert_eq!(layout.arkts, PathBuf::from("/sdk/ets/static/arkts"));
}

#[test]
fn split_dynamic_sdk_still_resolves_the_sibling_static_toolchain() {
    let config = SdkConfig {
        ets_loader_path: PathBuf::from("/sdk/ets/dynamic/build-tools/ets-loader"),
        es2abc_path: PathBuf::from(
            "/sdk/ets/dynamic/build-tools/ets-loader/bin/ark/build/bin/es2abc",
        ),
        restool_path: PathBuf::from("/sdk/toolchains/restool"),
        compile_api_version: 20,
        sdk_component_api_version: 20,
        has_rollup_plugin: true,
        compile_sdk_version: "7.0.0.1".to_string(),
    };

    let layout = config
        .static_arkts_layout()
        .expect("split static SDK layout");

    assert_eq!(layout.root, PathBuf::from("/sdk/ets/static"));
    assert_eq!(
        config.ets_api_dirs(),
        [PathBuf::from("/sdk/ets/dynamic/api")]
    );
    assert_eq!(
        config.ets_kits_dirs(),
        [PathBuf::from("/sdk/ets/dynamic/kits")]
    );
    assert_eq!(
        config.ets_arkts_dirs(),
        [PathBuf::from("/sdk/ets/dynamic/arkts")]
    );
    assert_eq!(
        config.ets_api_dirs_for(ArktsLanguageVersion::V1_2),
        [PathBuf::from("/sdk/ets/static/api")]
    );
    assert_eq!(
        config.ets_kits_dirs_for(ArktsLanguageVersion::V1_2),
        [PathBuf::from("/sdk/ets/static/kits")]
    );
    assert_eq!(
        config.ets_arkts_dirs_for(ArktsLanguageVersion::V1_2),
        [PathBuf::from("/sdk/ets/static/arkts")]
    );
}
