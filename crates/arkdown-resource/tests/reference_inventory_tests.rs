use std::fs;

use arkdown_resource::ResourceReferenceInventory;

#[test]
fn loads_resource_table_and_adjacent_rawfiles_like_sdk_loader() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("res");
    let rawfiles = output.join("resources/rawfile/config");
    fs::create_dir_all(&rawfiles).unwrap();
    fs::write(
        output.join("ResourceTable.txt"),
        concat!(
            "string title 0x01000001\n",
            "media logo 16777218\n",
            "invalid line\n",
            "color bad not-a-number\n"
        ),
    )
    .unwrap();
    fs::write(rawfiles.join("settings.json"), "{}").unwrap();

    let inventory =
        ResourceReferenceInventory::from_resource_table(&output.join("ResourceTable.txt")).unwrap();

    assert!(inventory.resources().contains("string.title"));
    assert!(inventory.resources().contains("media.logo"));
    assert!(!inventory.resources().contains("color.bad"));
    assert!(inventory.rawfiles().contains("config/settings.json"));
}

#[test]
fn loads_current_module_rawfile_directory_without_a_resource_table() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("images/icons")).unwrap();
    fs::write(temp.path().join("images/icons/add.svg"), "svg").unwrap();

    let inventory = ResourceReferenceInventory::from_rawfile_dir(temp.path()).unwrap();

    assert!(inventory.rawfiles().contains("images/icons/add.svg"));
    assert!(inventory.resources().is_empty());
}
