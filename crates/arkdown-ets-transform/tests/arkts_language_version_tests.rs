use arkdown_core::ArktsLanguageVersion;
use arkdown_ets_transform::CompileArktsSourceType;

#[test]
fn compile_source_type_defaults_to_arkts_1_1_and_only_explicitly_selects_1_2() {
    let default_source_type = CompileArktsSourceType::from_id("src/main/ets/Index.ets");
    assert!(default_source_type.is_arkui());
    assert!(!default_source_type.is_ets_static());

    let static_source_type = CompileArktsSourceType::from_id_with_version(
        "src/main/ets/Index.ets",
        ArktsLanguageVersion::V1_2,
    );
    assert!(static_source_type.is_ets_static());

    let declaration_source_type = CompileArktsSourceType::from_id_with_version(
        "src/main/ets/types.d.ets",
        ArktsLanguageVersion::V1_2,
    );
    assert!(!declaration_source_type.is_ets_static());
}
