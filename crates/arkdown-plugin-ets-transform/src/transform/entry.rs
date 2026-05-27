/// Generate the `@Entry` registration code appended after the ViewPU class.
///
/// `@Entry` registers the component as a page root. The SDK generates:
/// 1. `ViewStackProcessor.StartGetAccessRecordingFor(Name)`
/// 2. A static `create()` factory function
/// 3. `ViewStackProcessor.StopGetAccessRecording()`
///
/// This is appended after the class definition, not inside it.
pub fn generate_entry_registration(component_name: &str) -> String {
    format!(
        r#"ViewStackProcessor.StartGetAccessRecordingFor({name});
{name}.create = function(value?: object): void {{
  let __arkui_component__ = new {name}(undefined, value ?? {{}});
  ViewStackProcessor.GetAccessRecording()?.push(__arkui_component__);
}};
ViewStackProcessor.StopGetAccessRecording();
"#,
        name = component_name
    )
}
