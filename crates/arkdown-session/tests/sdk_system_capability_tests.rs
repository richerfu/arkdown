use std::collections::BTreeSet;

use arkdown_session::SdkSystemCapabilityInventory;

#[test]
fn sdk_system_capability_inventory_matches_checker_device_sets() {
    let temp = tempfile::tempdir().expect("temp dir");
    let core = temp.path().join("core/api/device-define");
    let extension = temp.path().join("extension/api/device-define");
    std::fs::create_dir_all(&core).expect("core device definitions");
    std::fs::create_dir_all(&extension).expect("extension device definitions");
    std::fs::write(
        core.join("default.json"),
        r#"{"SysCaps":["Common","Phone"]}"#,
    )
    .expect("phone definition");
    std::fs::write(
        core.join("tablet.json"),
        r#"{"SysCaps":["Common","Tablet"]}"#,
    )
    .expect("tablet definition");
    std::fs::write(
        extension.join("phone-extension.json"),
        r#"{"SysCaps":["Extension"]}"#,
    )
    .expect("external phone definition");

    let inventory = SdkSystemCapabilityInventory::from_api_roots(&[
        temp.path().join("core/api"),
        temp.path().join("extension/api"),
    ])
    .expect("system capability inventory");
    let selected = inventory
        .select(&["phone".to_string(), "tablet".to_string()])
        .expect("selected device capabilities");

    assert_eq!(
        selected
            .intersection
            .iter()
            .map(|value| value.to_string())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["Common".to_string()])
    );
    assert_eq!(
        selected
            .union
            .iter()
            .map(|value| value.to_string())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "Common".to_string(),
            "Extension".to_string(),
            "Phone".to_string(),
            "Tablet".to_string(),
        ])
    );
}
