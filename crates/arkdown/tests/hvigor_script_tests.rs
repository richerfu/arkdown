use std::fs;

use arkdown::HvigorScriptCompiler;

#[tokio::test]
async fn compiler_keeps_node_imports_external_and_embeds_oxc_helpers() {
    let fixture = tempfile::tempdir().expect("create fixture");
    let entry = fixture.path().join("hvigorfile.ts");
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

    let output = HvigorScriptCompiler::new(&entry)
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

    let output = HvigorScriptCompiler::new(&entry)
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
import { Input } from '@ohos/hvigor'
class DecoratedTask {
  @Input() flavor = 'default'
}
export default new DecoratedTask()
"#,
    )
    .expect("write decorated TypeScript entry");

    let output = HvigorScriptCompiler::new(&entry)
        .compile()
        .await
        .expect("compile decorated TypeScript entry");

    // DevEco Studio 6.1.1 / hvigor 6.24.3 uses TypeScript 4.9.5's
    // `__decorate(..., DecoratedTask.prototype, "flavor")` contract.
    assert!(output.contains("DecoratedTask.prototype"));
    assert!(output.contains("\"flavor\""));
    assert!(output.contains("Input)()"));
}

#[tokio::test]
async fn compiler_reports_rolldown_syntax_diagnostics_with_the_script_path() {
    let fixture = tempfile::tempdir().expect("create fixture");
    let entry = fixture.path().join("broken.ts");
    fs::write(&entry, "export default {\n").expect("write broken TypeScript entry");

    let error = HvigorScriptCompiler::new(&entry)
        .compile()
        .await
        .expect_err("reject broken TypeScript");
    let message = error.to_string();

    assert!(message.contains("failed to transpile"));
    assert!(message.contains("broken.ts"));
}
