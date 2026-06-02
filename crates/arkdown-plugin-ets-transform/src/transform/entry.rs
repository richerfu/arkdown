use std::fmt::Write;

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
    EntryRegistrationEmitter::new(component_name, info).emit()
}

struct EntryRegistrationEmitter<'a> {
    component_name: &'a str,
    info: Option<&'a EntryRegistrationInfo>,
}

impl<'a> EntryRegistrationEmitter<'a> {
    fn new(component_name: &'a str, info: Option<&'a EntryRegistrationInfo>) -> Self {
        Self {
            component_name,
            info,
        }
    }

    fn emit(&self) -> String {
        if let Some(info) = self.info.filter(|info| info.storage.is_some()) {
            return self.emit_card(info);
        }

        let metadata = self.metadata();
        let mut out = String::new();
        writeln!(
            out,
            "registerNamedRoute(() => new {}(undefined, {{}}), \"\", {metadata});",
            self.component_name,
        )
        .expect("writing to String cannot fail");
        out
    }

    fn emit_card(&self, info: &EntryRegistrationInfo) -> String {
        let storage = info.storage.as_deref().unwrap_or("undefined");
        let record_name = self.record_name(info);
        let mut out = String::new();
        out.push_str(
            "ViewStackProcessor.StartGetAccessRecordingFor(ViewStackProcessor.AllocateNewElmetIdForNextComponent());\n",
        );
        writeln!(
            out,
            "loadEtsCard(new {}(undefined, {{}}, {storage}), \"{}\");",
            self.component_name,
            escape_string(&record_name),
        )
        .expect("writing to String cannot fail");
        out.push_str("ViewStackProcessor.StopGetAccessRecording();\n");
        out
    }

    fn record_name(&self, info: &EntryRegistrationInfo) -> String {
        let mut out = String::new();
        write!(
            out,
            "{}/{}/ets/{}",
            info.bundle_name, info.module_name, info.page_path
        )
        .expect("writing to String cannot fail");
        out
    }

    fn metadata(&self) -> String {
        let Some(info) = self.info else {
            return "{}".to_string();
        };
        let mut out = String::new();
        write!(
            out,
            "{{ bundleName: \"{}\", moduleName: \"{}\", pagePath: \"{}\", pageFullPath: \"{}\", integratedHsp: \"{}\", moduleType: \"{}\" }}",
            escape_string(&info.bundle_name),
            escape_string(&info.module_name),
            escape_string(&info.page_path),
            escape_string(&info.page_full_path),
            escape_string(&info.integrated_hsp),
            escape_string(&info.module_type),
        )
        .expect("writing to String cannot fail");
        out
    }
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
