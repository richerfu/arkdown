use std::fs;

use arkdown::PluginModuleCompiler;

#[test]
fn cache_identity_tracks_configured_plugin_source_content() {
    let fixture = tempfile::tempdir().expect("create fixture");
    let entry = fixture.path().join("plugin.ts");
    fs::write(&entry, "export default { name: 'first' }\n").expect("write first plugin");
    let first = PluginModuleCompiler::new(&entry)
        .cache_identity()
        .expect("fingerprint first plugin");

    fs::write(&entry, "export default { name: 'second' }\n").expect("write second plugin");
    let second = PluginModuleCompiler::new(&entry)
        .cache_identity()
        .expect("fingerprint second plugin");

    assert_ne!(first, second);
    assert!(first.contains("plugin.ts#"));
}

#[tokio::test]
async fn compiler_keeps_node_imports_external_and_embeds_oxc_helpers() {
    let fixture = tempfile::tempdir().expect("create fixture");
    let entry = fixture.path().join("plugin.ts");
    fs::write(
        &entry,
        r#"
import { value } from './not-present'
interface Plugin { pluginId: string; apply(): number }
const base = { apply: () => value }
const plugin: Plugin = { ...base, pluginId: 'rust-compiled' }
export default plugin
"#,
    )
    .expect("write TypeScript entry");

    let output = PluginModuleCompiler::new(&entry)
        .compile()
        .await
        .expect("compile TypeScript entry");

    assert!(output.contains("require(\"./not-present\")"));
    assert!(!output.contains("@oxc-project/runtime"));
    assert!(!output.contains("interface Plugin"));
    assert!(!output.contains(&entry.to_string_lossy().to_string()));
    assert!(output.contains("__esModule"));
}

#[tokio::test]
async fn compiler_emits_an_inline_source_map_when_requested() {
    let fixture = tempfile::tempdir().expect("create fixture");
    let entry = fixture.path().join("plugin.ts");
    fs::write(&entry, "export default { pluginId: 'mapped' }\n").expect("write TypeScript entry");

    let output = PluginModuleCompiler::new(&entry)
        .with_source_map(true)
        .compile()
        .await
        .expect("compile TypeScript entry");

    assert!(output.contains("sourceMappingURL=data:application/json"));
}

#[tokio::test]
async fn compiler_preserves_typescript_legacy_field_decorator_semantics() {
    let fixture = tempfile::tempdir().expect("create fixture");
    let entry = fixture.path().join("decorated.ts");
    fs::write(
        &entry,
        r#"
const Input = () => (_target: object, _propertyKey: string) => undefined
class DecoratedTask {
  @Input() flavor = 'default'
}
export default new DecoratedTask()
"#,
    )
    .expect("write decorated TypeScript entry");

    let output = PluginModuleCompiler::new(&entry)
        .compile()
        .await
        .expect("compile decorated TypeScript entry");

    // Existing OHOS TypeScript plugins use the legacy
    // `__decorate(..., DecoratedTask.prototype, "flavor")` contract.
    assert!(output.contains("DecoratedTask.prototype"));
    assert!(output.contains("\"flavor\""));
    assert!(!output.contains("@Input"));
}

#[tokio::test]
async fn compiler_reports_rolldown_syntax_diagnostics_with_the_script_path() {
    let fixture = tempfile::tempdir().expect("create fixture");
    let entry = fixture.path().join("broken.ts");
    fs::write(&entry, "export default {\n").expect("write broken TypeScript entry");

    let error = PluginModuleCompiler::new(&entry)
        .compile()
        .await
        .expect_err("reject broken TypeScript");
    let message = error.to_string();

    assert!(message.contains("failed to transpile"));
    assert!(message.contains("broken.ts"));
}
