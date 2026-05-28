use crate::transform::decorator::{
    has_update_state_vars_slot, is_state_decorator, state_decorator_class,
};
use crate::transform::entry::{generate_entry_registration, EntryRegistrationInfo};

/// A state/prop/link field extracted from a `@Component` struct.
#[derive(Debug, Clone)]
pub struct StateField {
    /// Field name (without leading `__`).
    pub name: String,
    /// TypeScript type annotation text, e.g. `"number"` or `"string[]"`.
    pub type_annotation: String,
    /// Decorator name: `"State"`, `"Prop"`, `"Link"`, etc.
    pub decorator: String,
    /// Initializer expression text (may be empty string if absent).
    pub initializer: Option<String>,
}

/// Structural description of a parsed `@Component` struct, ready for code generation.
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: String,
    pub is_entry: bool,
    pub state_fields: Vec<StateField>,
    /// Lowered body for the hvigor-style `initialRender()` method.
    pub initial_render_body: String,
    pub entry_registration: Option<EntryRegistrationInfo>,
}

/// Generate the full ViewPU class text for a component.
///
/// The output is standard TypeScript that the downstream `arkdown-loader`
/// can parse and bundle without any ArkTS grammar extensions.
pub fn generate_view_pu_class(info: &ComponentInfo) -> String {
    let mut out = String::with_capacity(512);

    if info.is_entry {
        out.push_str(concat!(
            "if (!(\"finalizeConstruction\" in ViewPU.prototype)) {\n",
            "  Reflect.set(ViewPU.prototype, \"finalizeConstruction\", () => {});\n",
            "}\n\n",
        ));
    }

    // ── Class header ─────────────────────────────────────────────────────────
    out.push_str(&format!("class {} extends ViewPU {{\n", info.name));

    // ── Private backing fields ────────────────────────────────────────────────
    for field in &info.state_fields {
        let backing = state_decorator_class(&field.decorator);
        let type_param = if field.type_annotation.is_empty() {
            String::new()
        } else {
            format!("<{}>", field.type_annotation)
        };
        out.push_str(&format!(
            "  private __{name}: {backing}{type_param};\n",
            name = field.name,
            backing = backing,
            type_param = type_param,
        ));
    }

    // ── Getter / setter pairs ─────────────────────────────────────────────────
    for field in &info.state_fields {
        if is_state_decorator(&field.decorator) {
            let ty = &field.type_annotation;
            let ret_ty = if ty.is_empty() {
                "unknown".to_string()
            } else {
                ty.clone()
            };
            out.push_str(&format!(
                "  get {name}(): {ret_ty} {{ return this.__{name}.get(); }}\n",
                name = field.name,
                ret_ty = ret_ty,
            ));
            out.push_str(&format!(
                "  set {name}(v: {ret_ty}) {{ this.__{name}.set(v); }}\n",
                name = field.name,
                ret_ty = ret_ty,
            ));
        }
    }

    // ── Constructor ───────────────────────────────────────────────────────────
    //
    // Param names follow the ViewPU constructor signature from the OHOS SDK.
    out.push_str(concat!(
        "  constructor(\n",
        "    parent: ViewPU,\n",
        "    params: object,\n",
        "    localStorage?: LocalStorage,\n",
        "    elmtId: number = -1,\n",
        "    paramsLambda?: () => object,\n",
        "    extraInfo?: ExtraInfo,\n",
        "  ) {\n",
        "    super(parent, localStorage, elmtId, extraInfo);\n",
    ));
    for field in &info.state_fields {
        let backing = state_decorator_class(&field.decorator);
        let ty = &field.type_annotation;
        let init = field.initializer.as_deref().unwrap_or("undefined");
        let type_param = if ty.is_empty() {
            String::new()
        } else {
            format!("<{}>", ty)
        };
        out.push_str(&format!(
            "    this.__{name} = new {backing}{type_param}({init}, this, \"{name}\");\n",
            name = field.name,
            backing = backing,
            type_param = type_param,
            init = init,
        ));
    }
    out.push_str("    this.setInitiallyProvidedValue(params);\n");
    out.push_str("    this.finalizeConstruction();\n  }\n");

    // ── setInitiallyProvidedValue ─────────────────────────────────────────────
    out.push_str("  setInitiallyProvidedValue(params: object): void {\n");
    for field in &info.state_fields {
        out.push_str(&format!(
            "    if (params.{name} !== undefined) {{ this.{name} = params.{name}; }}\n",
            name = field.name,
        ));
    }
    out.push_str("  }\n");

    // ── updateStateVars ───────────────────────────────────────────────────────
    let prop_fields: Vec<&StateField> = info
        .state_fields
        .iter()
        .filter(|f| has_update_state_vars_slot(&f.decorator))
        .collect();

    let param_ty = build_params_type(&prop_fields);
    out.push_str(&format!(
        "  updateStateVars(params: Partial<{{{}}}>): void {{\n",
        param_ty
    ));
    for field in &prop_fields {
        out.push_str(&format!(
            "    if (params.{name} !== undefined) {{ this.__{name}.set(params.{name}); }}\n",
            name = field.name,
        ));
    }
    out.push_str("  }\n");

    // ── purgeVariableDependenciesOnElmtId ─────────────────────────────────────
    out.push_str("  purgeVariableDependenciesOnElmtId(rmElmtId: number): void {\n");
    for field in &info.state_fields {
        out.push_str(&format!(
            "    this.__{name}.purgeDependencyOnElmtId(rmElmtId);\n",
            name = field.name,
        ));
    }
    out.push_str("  }\n");

    // ── aboutToBeDeleted ──────────────────────────────────────────────────────
    out.push_str("  aboutToBeDeleted(): void {\n");
    for field in &info.state_fields {
        out.push_str(&format!(
            "    this.__{name}.aboutToBeDeleted();\n",
            name = field.name,
        ));
    }
    out.push_str(concat!(
        "    SubscriberManager.Get().delete(this.id__());\n",
        "    this.aboutToBeDeletedInternal();\n",
        "  }\n",
    ));

    // ── initialRender ─────────────────────────────────────────────────────────
    out.push_str("  initialRender(): void {\n");
    out.push_str(&info.initial_render_body);
    out.push_str("  }\n");

    // ── rerender ──────────────────────────────────────────────────────────────
    out.push_str("  rerender(): void { this.updateDirtyElements(); }\n");

    if info.is_entry {
        out.push_str(&format!(
            "  static getEntryName(): string {{ return \"{}\"; }}\n",
            info.name
        ));
    }

    out.push_str("}\n");

    // ── @Entry registration ───────────────────────────────────────────────────
    if info.is_entry {
        out.push('\n');
        out.push_str(&generate_entry_registration(
            &info.name,
            info.entry_registration.as_ref(),
        ));
    }

    out
}

/// Build the `Partial<{ field: type; ... }>` parameter type text for the
/// `updateStateVars` method.
fn build_params_type(fields: &[&StateField]) -> String {
    if fields.is_empty() {
        return String::new();
    }
    fields
        .iter()
        .map(|f| {
            if f.type_annotation.is_empty() {
                format!("{}: unknown", f.name)
            } else {
                format!("{}: {}", f.name, f.type_annotation)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_counter_component() -> ComponentInfo {
        ComponentInfo {
            name: "Counter".to_string(),
            is_entry: false,
            state_fields: vec![StateField {
                name: "count".to_string(),
                type_annotation: "number".to_string(),
                decorator: "State".to_string(),
                initializer: Some("0".to_string()),
            }],
            initial_render_body: concat!(
                "    this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
                "      Text.create(`${this.count}`);\n",
                "    }, Text);\n",
                "    Text.pop();\n",
            )
            .to_string(),
            entry_registration: None,
        }
    }

    #[test]
    fn test_generates_view_pu_class() {
        let info = make_counter_component();
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("class Counter extends ViewPU"),
            "missing class header"
        );
        assert!(
            out.contains("private __count: ObservedPropertySimplePU<number>"),
            "missing backing field"
        );
        assert!(out.contains("get count(): number"), "missing getter");
        assert!(out.contains("set count(v: number)"), "missing setter");
        assert!(
            out.contains("new ObservedPropertySimplePU<number>(0, this, \"count\")"),
            "missing ctor init"
        );
        assert!(
            out.contains("setInitiallyProvidedValue(params)"),
            "missing initial param override"
        );
        assert!(
            out.contains("finalizeConstruction()"),
            "missing finalizeConstruction"
        );
        assert!(
            out.contains("initialRender(): void"),
            "missing initialRender"
        );
        assert!(
            out.contains("this.observeComponentCreation2"),
            "missing lowered build body"
        );
        assert!(out.contains("rerender(): void"), "missing rerender");
        assert!(
            !out.contains("registerNamedRoute"),
            "@Entry registration must not appear for non-entry"
        );
    }

    #[test]
    fn test_entry_generates_registration() {
        let mut info = make_counter_component();
        info.is_entry = true;
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("registerNamedRoute(() => new Counter(undefined, {})"),
            "missing entry registration"
        );
        assert!(
            out.contains("static getEntryName(): string"),
            "missing getEntryName"
        );
    }

    #[test]
    fn test_prop_field_generates_update_state_vars() {
        let info = ComponentInfo {
            name: "Child".to_string(),
            is_entry: false,
            state_fields: vec![StateField {
                name: "value".to_string(),
                type_annotation: "string".to_string(),
                decorator: "Prop".to_string(),
                initializer: None,
            }],
            initial_render_body: String::new(),
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(out.contains("private __value: SynchedPropertySimpleOneWayPU<string>"));
        assert!(
            out.contains("params.__value") || out.contains("params.value"),
            "updateStateVars should reference prop field"
        );
    }

    #[test]
    fn test_empty_component() {
        let info = ComponentInfo {
            name: "Empty".to_string(),
            is_entry: false,
            state_fields: vec![],
            initial_render_body: String::new(),
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(out.contains("class Empty extends ViewPU"));
        assert!(out.contains("rerender(): void"));
    }
}
