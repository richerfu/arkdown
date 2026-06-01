/// Route metadata used by hvigor's generated `registerNamedRoute` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRegistrationInfo {
    pub bundle_name: String,
    pub module_name: String,
    pub page_path: String,
    pub page_full_path: String,
    pub integrated_hsp: String,
    pub module_type: String,
    pub storage: Option<String>,
}

/// Generate the `@Entry` registration code appended after the ViewPU class.
///
/// The minimal fixture's hvigor output registers page roots with
/// `registerNamedRoute(() => new Name(undefined, {}), "", metadata)`.
pub fn generate_entry_registration(
    component_name: &str,
    info: Option<&EntryRegistrationInfo>,
) -> String {
    if let Some(info) = info.filter(|info| info.storage.is_some()) {
        let storage = info.storage.as_deref().unwrap_or("undefined");
        let record_name = format!(
            "{}/{}/ets/{}",
            info.bundle_name, info.module_name, info.page_path
        );
        return format!(
            concat!(
                "ViewStackProcessor.StartGetAccessRecordingFor(ViewStackProcessor.AllocateNewElmetIdForNextComponent());\n",
                "loadEtsCard(new {name}(undefined, {{}}, {storage}), \"{record_name}\");\n",
                "ViewStackProcessor.StopGetAccessRecording();\n"
            ),
            name = component_name,
            storage = storage,
            record_name = escape_string(&record_name),
        );
    }

    let metadata = if let Some(info) = info {
        format!(
            concat!(
                "{{ ",
                "bundleName: \"{bundle_name}\", ",
                "moduleName: \"{module_name}\", ",
                "pagePath: \"{page_path}\", ",
                "pageFullPath: \"{page_full_path}\", ",
                "integratedHsp: \"{integrated_hsp}\", ",
                "moduleType: \"{module_type}\" ",
                "}}"
            ),
            bundle_name = escape_string(&info.bundle_name),
            module_name = escape_string(&info.module_name),
            page_path = escape_string(&info.page_path),
            page_full_path = escape_string(&info.page_full_path),
            integrated_hsp = escape_string(&info.integrated_hsp),
            module_type = escape_string(&info.module_type),
        )
    } else {
        "{}".to_string()
    };

    format!(
        "registerNamedRoute(() => new {name}(undefined, {{}}), \"\", {metadata});\n",
        name = component_name,
        metadata = metadata,
    )
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
