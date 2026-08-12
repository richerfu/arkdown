use arkdown_config::{ArkdownCustomConfigLoader, ConfigError};

#[test]
fn toml_build_profile_fields_preserve_order_and_types() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("arkdown.config.toml");
    std::fs::write(
        &path,
        r#"
shimMissingExports = true

[build-profile]
API_HOST = "https://arkdown.example"
FEATURE_FLAG = false
RETRY_COUNT = 3
"#,
    )
    .expect("write TOML config");

    let config = ArkdownCustomConfigLoader::new(temp.path())
        .load()
        .expect("load TOML config");

    assert_eq!(config.source_path(), Some(path.as_path()));
    assert_eq!(config.shim_missing_exports, Some(true));
    assert_eq!(
        config
            .build_profile_fields
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["API_HOST", "FEATURE_FLAG", "RETRY_COUNT"]
    );
    assert_eq!(
        config.build_profile_fields["API_HOST"],
        "https://arkdown.example"
    );
    assert_eq!(config.build_profile_fields["FEATURE_FLAG"], false);
    assert_eq!(config.build_profile_fields["RETRY_COUNT"], 3);
}

#[test]
fn toml_is_authoritative_and_json_remains_a_fallback() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("arkdown.config.json"),
        r#"{"buildProfileFields":{"SOURCE":"json"}}"#,
    )
    .expect("write JSON config");

    let json_config = ArkdownCustomConfigLoader::new(temp.path())
        .load()
        .expect("load JSON fallback");
    assert_eq!(json_config.build_profile_fields["SOURCE"], "json");

    let toml_path = temp.path().join("arkdown.config.toml");
    std::fs::write(&toml_path, "[build-profile]\nSOURCE = \"toml\"\n").expect("write TOML config");
    let toml_config = ArkdownCustomConfigLoader::new(temp.path())
        .load()
        .expect("load authoritative TOML");
    assert_eq!(toml_config.source_path(), Some(toml_path.as_path()));
    assert_eq!(toml_config.build_profile_fields["SOURCE"], "toml");
}

#[test]
fn non_primitive_toml_build_profile_field_is_rejected_at_its_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("arkdown.config.toml");
    std::fs::write(&path, "[build-profile]\nINVALID = [1, 2]\n").expect("write TOML config");

    let error = ArkdownCustomConfigLoader::new(temp.path())
        .load()
        .expect_err("array build-profile field must be rejected");

    let ConfigError::ValidationError { file, message } = error else {
        panic!("expected validation error");
    };
    assert_eq!(file, path);
    assert!(message.contains("build-profile.INVALID"), "{message}");
    assert!(message.contains("array"), "{message}");
}
