# SPEC-026: ArkTS Widget Compile Pass

**Crate**: `arkdown-abc` (extension) + `arkdown-cli` (wiring)
**Priority**: P2
**Blocked by**: SPEC-009
**Status**: Implemented (2026-05-29)

---

## Overview

ArkTS widgets (form extension abilities) require a **second, independent ABC
compilation pass** that runs after the main module compilation.  This mirrors
`ArkCompile.doTaskAction()` in hvigor's `ark-compile.js`:

```javascript
if (this.needSubmitArkTsWidget) {
    const widgetConfig = {
        ...config,
        widgetCompile: "true",
        cachePath:    taskTempDir + "_widget",
        aceBuildJson: pathInfo.getWidgetLoaderJsonPath(),
        byteCodeHar2HspDep: this.service.getByteCodeHar2Hsp(),
    };
    widgetConfig.ignoreWarning = true;
    await submitArkCompileWork(...widgetConfig...);
}
```

---

## Activation Condition

The widget pass runs when **both** conditions hold:

1. The module declares at least one extension ability whose `extensionType` is
   `"form"` or `"formExtension"` (case-insensitive).
2. The module type is **not** `"har"` — HAR modules cannot expose widgets.

```rust
let need = need_widget_compile(&form_json_paths, module_type);
```

This mirrors `shouldCompileArkTsWidget(formJsonPathArr, isFormExtensionModule)` in
hvigor's `task-util.js`.

---

## Implementation

### `arkdown-abc::widget` module

New public module at `crates/arkdown-abc/src/widget.rs`.

Public API:

| Symbol | Description |
|--------|-------------|
| `WidgetCompileConfig` | Input configuration for the widget pass |
| `WidgetInput` | One widget source file to compile |
| `need_widget_compile(paths, module_type)` | Activation predicate |
| `collect_form_json_paths(src_main, ext_abilities)` | Collect `form.json` paths from extension abilities |
| `ExtensionAbilityRef<'a>` | Lightweight borrow of an extension ability entry |

### `AbcCompiler::compile_widget`

New method on `AbcCompiler` in `crates/arkdown-abc/src/compiler.rs`:

```rust
pub fn compile_widget(
    &self,
    config: &WidgetCompileConfig,
) -> Result<AbcArtifactResult, AbcError>
```

Behaviour:
- Creates the widget cache directory (`config.cache_dir`) if it does not exist.
- Builds a **fresh `AbcFile`** independent from the main module's `AbcFile`.
- Skips missing source files (hvigor sets `ignoreWarning: true`).
- Writes the compiled bytecode to `config.output` (typically `widgets.abc`).

### CLI wiring (`arkdown-cli`)

`compile_widget_pass()` is called from the ABC compilation step in
`crates/arkdown-cli/src/cmd/build.rs` after the main `compile_to_abc()` call.

```
main ABC pass (modules.abc)
    ↓
need_widget_compile? ──yes──► compile_widget_pass() → widgets.abc
                    └──no──►  (skip)
```

Key path decisions:

| Item | Path |
|------|------|
| Widget cache dir | `<loader_json.cache_path>_widget/` |
| Widget output | `<modules_abc_path_dir>/widgets.abc` |
| Widget source | `.js` chunks whose stem matches a form extension `src_entry` stem |

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
|-----------------|---------------------|------|
| `AbcCompiler::compile_widget()` | rolldown multi-entry build with separate `output.dir` | Both run a separate compilation pass; rolldown uses separate output options; arkdown uses a separate cache dir and loader.json |

---

## File Locations

| File | Change |
|------|--------|
| `crates/arkdown-abc/src/widget.rs` | New module — widget config, activation predicate, form path collection |
| `crates/arkdown-abc/src/compiler.rs` | Added `compile_widget()` method to `AbcCompiler` |
| `crates/arkdown-abc/src/lib.rs` | Re-exported `widget` module and public symbols |
| `crates/arkdown-cli/src/cmd/build.rs` | Wired `compile_widget_pass()` into the ABC compilation step |

---

## Acceptance Criteria

- `compile_widget()` is called when `need_widget_compile = true` (form extension
  abilities present in `module.json5`).
- Widget compilation uses a separate cache directory from the main module
  compilation (suffix `_widget`).
- Widget `.abc` output is placed at `widgets.abc` alongside `modules.abc`, not
  overwriting the main `modules.abc`.
- `compile_widget()` is never called for HAR modules (`module.type_ == "har"`).
- Missing widget source files are silently skipped (hvigor `ignoreWarning: true`
  parity).
- All existing tests continue to pass.

---

## Tests

Unit tests in `crates/arkdown-abc/src/widget.rs`:

| Test | What it checks |
|------|----------------|
| `need_widget_compile_true_when_form_paths_present` | Returns `true` for entry/feature/shared with form paths |
| `need_widget_compile_false_for_har_module` | Returns `false` for HAR even with form paths |
| `need_widget_compile_false_when_no_form_paths` | Returns `false` when no form paths |
| `need_widget_compile_false_for_har_with_no_paths` | Both conditions false |
| `collect_form_json_paths_empty_when_no_form_extensions` | Non-form extensions produce no paths |
| `collect_form_json_paths_case_insensitive` | `"Form"` (capitalised) matches |
