use arkdown_core::ArktsLanguageVersion;
use tempfile::TempDir;

use crate::bundle_plan::RolldownCompileEntryPlan;

#[test]
fn static_runtime_class_entry_is_not_submitted_to_rolldown() {
    let temp = TempDir::new().expect("create entry fixture");
    let source_root = temp.path().join("entry/src/main");
    let ability = source_root.join("ets/entryability/EntryAbility.ets");
    std::fs::create_dir_all(ability.parent().expect("ability directory"))
        .expect("create ability directory");
    std::fs::write(
        &ability,
        "'use static';\nexport default class EntryAbility {}\n",
    )
    .expect("write ability source");
    let resource_table = temp.path().join("build/ResourceTable.ts");
    std::fs::create_dir_all(resource_table.parent().expect("resource table directory"))
        .expect("create resource table directory");
    std::fs::write(&resource_table, "export const app_name = 0;\n").expect("write resource table");

    let serialized = vec![resource_table.display().to_string()];
    let runtime_class_entry = "entry/src/main/ets/entryability/EntryAbility".to_string();
    let source_entry = "./ets/entryability/EntryAbility.ets".to_string();
    let internal = vec![runtime_class_entry.clone(), source_entry.clone()];
    let runtime = internal.clone();

    let static_entries = RolldownCompileEntryPlan::new(
        &serialized,
        &internal,
        &runtime,
        &source_root,
        ArktsLanguageVersion::V1_2,
    )
    .collect();

    assert_eq!(
        static_entries,
        vec![resource_table.display().to_string(), source_entry.clone()],
        "interop/main.js::setAbilityFile skips the runtime-only static class path but keeps a real source entry"
    );

    let dynamic_entries = RolldownCompileEntryPlan::new(
        &serialized,
        &internal,
        &runtime,
        &source_root,
        ArktsLanguageVersion::V1_1,
    )
    .collect();
    assert_eq!(
        dynamic_entries,
        vec![
            resource_table.display().to_string(),
            runtime_class_entry,
            source_entry,
        ],
        "the source skip is gated by interop isMixCompile and must not alter ArkTS 1.1 diagnostics"
    );
}
