use crate::transform::build_fn::SpanHint;
use crate::transform::component_field::{
    push_regular_field_declaration, push_state_constructor_init, push_state_field_declaration,
};
use crate::transform::decorator::{has_initially_provided_value_slot, has_update_state_vars_slot};
use crate::transform::entry::{generate_entry_registration, EntryRegistrationInfo};

/// A `@Builder` method extracted from a `@Component` struct.
///
/// In ArkTS, `@Builder` methods contain the same ArkUI component DSL as `build()`.
/// They remain as class methods on the ViewPU subclass but their bodies are desugared
/// to `observeComponentCreation2` calls (identical to `build()` desugaring).
#[derive(Debug, Clone)]
pub struct BuilderMethodInfo {
    /// Method name as declared (not renamed).
    pub name: String,
    /// Parameter list text (without outer parens), e.g. `"text: string"`.
    pub params: String,
    /// Return type annotation text including the leading colon, if present.
    pub return_annotation: String,
    /// ArkUI DSL body already lowered via `lower_build_body`.
    pub lowered_body: String,
}

/// A `@Styles` method extracted from a `@Component` struct.
///
/// In ArkTS, `@Styles` methods declare reusable attribute chains (`.width()`, `.height()`,
/// etc.).  The decorator is stripped and the method becomes `private`.  The attribute chain
/// body is lowered: `.attr(x)` → `this.attr(x)`.
#[derive(Debug, Clone)]
pub struct StylesMethodInfo {
    /// Method name as declared.
    pub name: String,
    /// Parameter list text (without outer parens), typically empty `""`.
    pub params: String,
    /// Lowered body: `.attr(x)` chains rewritten to `this.attr(x)` statements.
    pub lowered_body: String,
}

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
    /// Handler name from `@Watch("handlerName")`, if present on this field.
    /// When set, generates `this.declareWatch("fieldName", this.handlerName)` in the
    /// constructor (after `setInitiallyProvidedValue`).
    pub watch_handler: Option<String>,
    /// Alias key from `@Provide("alias")` / `@Consume("alias")`.
    /// For `@Provide`, this is the key exposed to descendants.
    /// For `@Consume`, this is the key looked up from an ancestor `@Provide`.
    /// When absent the field name itself is used as the key.
    pub alias: Option<String>,
    /// First decorator argument expression text for decorators that pass the
    /// original expression through to runtime APIs, such as `@StorageProp`.
    pub decorator_arg: Option<String>,
    /// Result of hvigor's `isSimpleType` equivalent for this field, derived
    /// from the OXC type annotation AST.
    pub is_simple_type: bool,
}

#[derive(Debug, Clone)]
pub struct ParamField {
    pub name: String,
    pub type_annotation: String,
}

#[derive(Debug, Clone)]
pub struct RegularField {
    pub name: String,
    pub type_annotation: String,
    pub initializer: Option<String>,
    pub optional: bool,
}

/// Structural description of a parsed `@Component` struct, ready for code generation.
#[derive(Debug, Clone, Default)]
pub struct ComponentInfo {
    pub name: String,
    pub is_entry: bool,
    pub is_exported: bool,
    pub is_default_export: bool,
    pub is_reusable: bool,
    pub state_fields: Vec<StateField>,
    /// All struct properties in source order. hvigor adds every property to
    /// the generated `*_Params` interface from `createVariableInitStatement`,
    /// including regular/private fields and callback fields.
    pub params_fields: Vec<ParamField>,
    /// Struct property names in source order. Constructor initialization,
    /// `setInitiallyProvidedValue`, and field declarations follow this order.
    pub property_order: Vec<String>,
    /// Non-decorated struct fields. hvigor still routes them through params and
    /// emits private class fields initialized in the constructor.
    pub regular_fields: Vec<RegularField>,
    /// Struct members that are not ArkUI state/decorator fields and are not
    /// build/@Builder/@Styles methods. hvigor keeps these as class members.
    pub regular_members: Vec<String>,
    /// Lowered body for the hvigor-style `initialRender()` method.
    pub initial_render_body: String,
    /// Span hints collected during ArkUI DSL lowering.  Each hint records a
    /// 0-based line index within `initial_render_body` and the original `.ets`
    /// span of the component expression that produced that line.  Used to
    /// build a cross-transform source map in the plugin layer.
    pub initial_render_span_hints: Vec<SpanHint>,
    pub entry_registration: Option<EntryRegistrationInfo>,
    /// `@Builder` methods: desugared ArkUI DSL, emitted as regular class methods.
    pub builder_methods: Vec<BuilderMethodInfo>,
    /// `@Styles` methods: emitted as `private` class methods with lowered body.
    pub styles_methods: Vec<StylesMethodInfo>,
}

/// Generate the full ViewPU class text for a component.
///
/// The output is standard TypeScript that the downstream `arkdown-loader`
/// can parse and bundle without any ArkTS grammar extensions.
pub fn generate_view_pu_class(info: &ComponentInfo) -> String {
    let mut out = String::with_capacity(512);
    let fallback_property_order;
    let property_order = if info.property_order.is_empty() {
        fallback_property_order = info
            .state_fields
            .iter()
            .map(|field| field.name.clone())
            .chain(info.regular_fields.iter().map(|field| field.name.clone()))
            .collect::<Vec<_>>();
        &fallback_property_order
    } else {
        &info.property_order
    };

    let params_type = format!("{}_Params", info.name);
    out.push_str(&format!("interface {params_type} {{\n"));
    let state_params;
    let params_fields = if info.params_fields.is_empty() {
        state_params = info
            .state_fields
            .iter()
            .map(|field| ParamField {
                name: field.name.clone(),
                type_annotation: field.type_annotation.clone(),
            })
            .collect::<Vec<_>>();
        &state_params
    } else {
        &info.params_fields
    };
    for field in params_fields {
        if field.type_annotation.is_empty() {
            out.push_str(&format!("    {}?;\n", field.name));
        } else {
            out.push_str(&format!(
                "    {}?: {};\n",
                field.name, field.type_annotation
            ));
        }
    }
    out.push_str("}\n");

    let export_prefix = if info.is_default_export {
        "export default "
    } else if info.is_exported {
        "export "
    } else {
        ""
    };
    out.push_str(&format!(
        "{export_prefix}class {} extends ViewPU {{\n",
        info.name
    ));

    out.push_str(concat!(
        "    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {\n",
        "        super(parent, __localStorage, elmtId, extraInfo);\n",
        "        if (typeof paramsLambda === \"function\") {\n",
        "            this.paramsGenerator_ = paramsLambda;\n",
        "        }\n",
    ));
    for name in property_order {
        if let Some(field) = find_regular_field(info, name) {
            let init = field.initializer.as_deref().unwrap_or("undefined");
            out.push_str(&format!("        this.{name} = {init};\n"));
        } else if let Some(field) = find_state_field(info, name) {
            push_state_constructor_init(&mut out, field);
        }
    }
    out.push_str("        this.setInitiallyProvidedValue(params);\n");
    // @Watch — hvigor appends declareWatch calls after setInitiallyProvidedValue.
    for field in &info.state_fields {
        if let Some(handler) = &field.watch_handler {
            out.push_str(&format!(
                "        this.declareWatch(\"{name}\", this.{handler});\n",
                name = field.name,
                handler = handler,
            ));
        }
    }
    out.push_str("        this.finalizeConstruction();\n    }\n");

    out.push_str(&format!(
        "    setInitiallyProvidedValue(params: {params_type}) {{\n"
    ));
    for name in property_order {
        if find_regular_field(info, name).is_some()
            || find_state_field(info, name)
                .is_some_and(|field| has_initially_provided_value_slot(&field.decorator))
        {
            out.push_str(&format!(
                "        if (params.{name} !== undefined) {{\n            this.{name} = params.{name};\n        }}\n",
            ));
        }
    }
    out.push_str("    }\n");

    let prop_fields: Vec<&StateField> = info
        .state_fields
        .iter()
        .filter(|f| has_update_state_vars_slot(&f.decorator))
        .collect();

    out.push_str(&format!("    updateStateVars(params: {params_type}) {{\n"));
    for field in &prop_fields {
        if field.decorator == "State" {
            out.push_str(&format!(
                "        if (params.{name}) {{\n            this.__{name}.set(params.{name});\n        }}\n",
                name = field.name,
            ));
        } else {
            let update_api = match field.decorator.as_str() {
                "Prop" | "Param" | "Once" | "PropRef" => "reset",
                _ => "set",
            };
            out.push_str(&format!(
                "        this.__{name}.{update_api}(params.{name});\n",
                name = field.name,
                update_api = update_api,
            ));
        }
    }
    out.push_str("    }\n");

    out.push_str("    purgeVariableDependenciesOnElmtId(rmElmtId) {\n");
    for field in &info.state_fields {
        if field.decorator == "BuilderParam" {
            continue;
        }
        out.push_str(&format!(
            "        this.__{name}.purgeDependencyOnElmtId(rmElmtId);\n",
            name = field.name,
        ));
    }
    out.push_str("    }\n");

    out.push_str("    aboutToBeDeleted() {\n");
    for field in &info.state_fields {
        if field.decorator == "BuilderParam" {
            continue;
        }
        out.push_str(&format!(
            "        this.__{name}.aboutToBeDeleted();\n",
            name = field.name,
        ));
    }
    out.push_str(concat!(
        "        SubscriberManager.Get().delete(this.id__());\n",
        "        this.aboutToBeDeletedInternal();\n",
        "    }\n",
    ));

    if info.is_reusable {
        out.push_str("    updateRecycleElmtId(oldElmtId, newElmtId) {\n");
        for field in &info.state_fields {
            if field.decorator == "BuilderParam" {
                continue;
            }
            out.push_str(&format!(
                "        this.__{name}.updateElmtId(oldElmtId, newElmtId);\n",
                name = field.name,
            ));
        }
        out.push_str("    }\n");
    }

    for name in property_order {
        if let Some(field) = find_regular_field(info, name) {
            push_regular_field_declaration(&mut out, field);
        } else if let Some(field) = find_state_field(info, name) {
            push_state_field_declaration(&mut out, field);
        }
    }

    for member in &info.regular_members {
        out.push_str(&render_regular_member(member));
    }

    out.push_str("    initialRender() {\n");
    out.push_str(&info.initial_render_body);
    out.push_str("    }\n");

    // ── @Builder methods ─────────────────────────────────────────────────────
    // Each @Builder method stays as a class method with the same name and its
    // ArkUI build() DSL body desugared to observeComponentCreation2 calls.
    for method in &info.builder_methods {
        let params = if method.params.is_empty() {
            String::new()
        } else {
            method.params.clone()
        };
        out.push_str(&format!(
            "    {}({}){} {{\n",
            method.name, params, method.return_annotation
        ));
        out.push_str(&method.lowered_body);
        out.push_str("    }\n");
    }

    // ── @Styles methods ──────────────────────────────────────────────────────
    // Each @Styles method becomes a private class method with the decorator
    // stripped.  The attribute chain body is lowered: `.attr(x)` → `this.attr(x)`.
    for method in &info.styles_methods {
        let params = if method.params.is_empty() {
            String::new()
        } else {
            method.params.clone()
        };
        out.push_str(&format!(
            "    private {}({}): void {{\n",
            method.name, params
        ));
        out.push_str(&method.lowered_body);
        out.push_str("    }\n");
    }

    out.push_str("    rerender() {\n        this.updateDirtyElements();\n    }\n");

    if info.is_entry {
        out.push_str("    static getEntryName(): string {\n");
        out.push_str(&format!("        return \"{}\";\n", info.name));
        out.push_str("    }\n");
    }

    out.push_str("}\n");

    // ── @Entry registration ───────────────────────────────────────────────────
    if info.is_entry {
        out.push_str(&generate_entry_registration(
            &info.name,
            info.entry_registration.as_ref(),
        ));
    }

    out
}

fn find_regular_field<'a>(info: &'a ComponentInfo, name: &str) -> Option<&'a RegularField> {
    info.regular_fields.iter().find(|field| field.name == name)
}

fn find_state_field<'a>(info: &'a ComponentInfo, name: &str) -> Option<&'a StateField> {
    info.state_fields.iter().find(|field| field.name == name)
}

fn render_regular_member(member: &str) -> String {
    let trimmed = member.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut lines = trimmed.lines().collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }

    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let is_final_closing_brace = idx + 1 == lines.len() && line.trim() == "}";
        out.push_str("    ");
        if is_final_closing_brace {
            out.push('}');
        } else if idx == 0 {
            out.push_str(line.trim_start());
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !trimmed.ends_with('}') && !trimmed.ends_with(';') {
        if out.ends_with('\n') {
            out.pop();
        }
        out.push_str(";\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_counter_component() -> ComponentInfo {
        ComponentInfo {
            name: "Counter".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "count".to_string(),
                type_annotation: "number".to_string(),
                decorator: "State".to_string(),
                initializer: Some("0".to_string()),
                watch_handler: None,
                alias: None,
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: concat!(
                "        this.observeComponentCreation2((elmtId, isInitialRender) => {\n",
                "            Text.create(`${this.count}`);\n",
                "        }, Text);\n",
                "        Text.pop();\n",
            )
            .to_string(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
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
        assert!(out.contains("get count()"), "missing getter");
        assert!(
            out.contains("set count(newValue: number)"),
            "missing setter"
        );
        assert!(
            out.contains("new ObservedPropertySimplePU(0, this, \"count\")"),
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
        assert!(out.contains("initialRender()"), "missing initialRender");
        assert!(
            out.contains("this.observeComponentCreation2"),
            "missing lowered build body"
        );
        assert!(out.contains("rerender()"), "missing rerender");
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
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "value".to_string(),
                type_annotation: "string".to_string(),
                decorator: "Prop".to_string(),
                initializer: None,
                watch_handler: None,
                alias: None,
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(out.contains("private __value: SynchedPropertySimpleOneWayPU<string>"));
        assert!(
            out.contains("this.__value.reset(params.value);"),
            "updateStateVars should directly reset @Prop like hvigor, got:\n{out}"
        );
        assert!(
            !out.contains("if (params.value !== undefined)"),
            "@Prop updateStateVars must not guard reset in hvigor PU output"
        );
    }

    #[test]
    fn test_empty_component() {
        let info = ComponentInfo {
            name: "Empty".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(out.contains("class Empty extends ViewPU"));
        assert!(out.contains("rerender()"));
    }

    // ── @Link ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_link_field_uses_params_init_and_two_way_backing_class() {
        let info = ComponentInfo {
            name: "Child".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "items".to_string(),
                type_annotation: "string[]".to_string(),
                decorator: "Link".to_string(),
                initializer: None,
                watch_handler: None,
                alias: None,
                decorator_arg: None,
                is_simple_type: false,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("private __items: SynchedPropertySimpleOneWayPU<string[]>"),
            "wrong declaration backing class for non-simple @Link"
        );
        assert!(
            out.contains("new SynchedPropertyObjectTwoWayPU(params.items, this, \"items\")"),
            "@Link must be initialised from params, not from a local literal"
        );
        // @Link does NOT appear in updateStateVars
        assert!(
            !out.contains("this.__items.set(params.items)"),
            "@Link must not have an updateStateVars slot"
        );
    }

    // ── @ObjectLink ───────────────────────────────────────────────────────────

    #[test]
    fn test_object_link_field_uses_params_init_and_nesed_object_backing_class() {
        let info = ComponentInfo {
            name: "ItemRow".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "item".to_string(),
                type_annotation: "Item".to_string(),
                decorator: "ObjectLink".to_string(),
                initializer: None,
                watch_handler: None,
                alias: None,
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("private __item: SynchedPropertyNesedObjectPU<Item>"),
            "wrong backing class for @ObjectLink"
        );
        assert!(
            out.contains("new SynchedPropertyNesedObjectPU(params.item, this, \"item\")"),
            "@ObjectLink must be initialised from params"
        );
    }

    #[test]
    fn test_reusable_component_generates_update_recycle_elmt_id() {
        let info = ComponentInfo {
            name: "ReusableRow".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: true,
            state_fields: vec![
                StateField {
                    name: "item".to_string(),
                    type_annotation: "Item".to_string(),
                    decorator: "ObjectLink".to_string(),
                    initializer: None,
                    watch_handler: None,
                    alias: None,
                    decorator_arg: None,
                    is_simple_type: false,
                },
                StateField {
                    name: "content".to_string(),
                    type_annotation: "() => void".to_string(),
                    decorator: "BuilderParam".to_string(),
                    initializer: None,
                    watch_handler: None,
                    alias: None,
                    decorator_arg: None,
                    is_simple_type: true,
                },
            ],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![RegularField {
                name: "local".to_string(),
                type_annotation: "number".to_string(),
                initializer: Some("1".to_string()),
                optional: false,
            }],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("updateRecycleElmtId(oldElmtId, newElmtId)"),
            "reusable component should emit updateRecycleElmtId, got:\n{out}"
        );
        assert!(
            out.contains("this.__item.updateElmtId(oldElmtId, newElmtId);"),
            "non-regular state fields should update element id"
        );
        assert!(
            !out.contains("this.__content.updateElmtId"),
            "@BuilderParam is immutable and should not update element id"
        );
        assert!(
            !out.contains("this.__local.updateElmtId"),
            "regular fields should not update element id"
        );
    }

    // ── @Provide ──────────────────────────────────────────────────────────────

    #[test]
    fn test_provide_field_uses_local_init_and_simple_pu_backing_class() {
        let info = ComponentInfo {
            name: "Parent".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "theme".to_string(),
                type_annotation: "string".to_string(),
                decorator: "Provide".to_string(),
                initializer: Some("\"dark\"".to_string()),
                watch_handler: None,
                alias: None,
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("private __theme: ObservedPropertySimplePU<string>"),
            "wrong backing class for @Provide"
        );
        assert!(
            out.contains("new ObservedPropertySimplePU(\"dark\", this, \"theme\")"),
            "@Provide must be initialised with the field's own initializer"
        );
    }

    // ── @Consume ──────────────────────────────────────────────────────────────

    #[test]
    fn test_consume_field_uses_initialize_consume_without_alias() {
        let info = ComponentInfo {
            name: "Child".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "theme".to_string(),
                type_annotation: "string".to_string(),
                decorator: "Consume".to_string(),
                initializer: None,
                watch_handler: None,
                alias: None,
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("private __theme: ObservedPropertyAbstractPU<string>"),
            "wrong backing class for @Consume"
        );
        assert!(
            out.contains("this.initializeConsume(\"theme\", \"theme\")"),
            "@Consume without alias must use the field name as the lookup key"
        );
    }

    #[test]
    fn test_consume_field_uses_alias_when_provided() {
        let info = ComponentInfo {
            name: "Child".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "currentTheme".to_string(),
                type_annotation: "string".to_string(),
                decorator: "Consume".to_string(),
                initializer: None,
                watch_handler: None,
                alias: Some("theme".to_string()),
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("this.initializeConsume(\"theme\", \"currentTheme\")"),
            "@Consume(\"theme\") must use the alias as the lookup key"
        );
    }

    // ── @Watch ────────────────────────────────────────────────────────────────

    #[test]
    fn test_watch_generates_declare_watch_call_in_constructor() {
        let info = ComponentInfo {
            name: "Watcher".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "count".to_string(),
                type_annotation: "number".to_string(),
                decorator: "State".to_string(),
                initializer: Some("0".to_string()),
                watch_handler: Some("onCountChange".to_string()),
                alias: None,
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        // The declareWatch call must appear AFTER setInitiallyProvidedValue, as
        // hvigor appends it after field init and initial param assignment.
        let declare_pos = out
            .find("this.declareWatch(\"count\", this.onCountChange)")
            .expect("missing declareWatch call");
        let init_pos = out
            .find("new ObservedPropertySimplePU(0, this, \"count\")")
            .expect("missing backing field init");
        let set_pos = out
            .find("this.setInitiallyProvidedValue(params)")
            .expect("missing setInitiallyProvidedValue");
        assert!(
            init_pos < declare_pos,
            "declareWatch must come after field init"
        );
        assert!(
            set_pos < declare_pos,
            "declareWatch must come after setInitiallyProvidedValue"
        );
    }

    #[test]
    fn test_watch_without_handler_emits_no_declare_watch() {
        let info = make_counter_component();
        let out = generate_view_pu_class(&info);
        assert!(
            !out.contains("declareWatch"),
            "@State without @Watch must not emit declareWatch"
        );
    }

    // ── @BuilderParam ─────────────────────────────────────────────────────────

    #[test]
    fn test_builder_param_uses_direct_this_assignment() {
        let info = ComponentInfo {
            name: "Card".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "content".to_string(),
                type_annotation: "() => void".to_string(),
                decorator: "BuilderParam".to_string(),
                initializer: None,
                watch_handler: None,
                alias: None,
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("private __content?"),
            "wrong backing field declaration for @BuilderParam"
        );
        // @BuilderParam is assigned onto this.name (no __ prefix) in the constructor.
        assert!(
            out.contains("this.content = undefined"),
            "@BuilderParam without initializer must assign undefined"
        );
        assert!(
            !out.contains("this.__content.set(params.content)"),
            "@BuilderParam must not have an updateStateVars slot in hvigor PU output"
        );
    }

    // ── Prop constructor init via params ───────────────────────────────────────

    #[test]
    fn test_prop_constructor_uses_params_not_local_init() {
        let info = ComponentInfo {
            name: "Child".to_string(),
            is_entry: false,
            is_exported: false,
            is_default_export: false,
            is_reusable: false,
            state_fields: vec![StateField {
                name: "title".to_string(),
                type_annotation: "string".to_string(),
                decorator: "Prop".to_string(),
                initializer: Some("\"default\"".to_string()),
                watch_handler: None,
                alias: None,
                decorator_arg: None,
                is_simple_type: true,
            }],
            params_fields: vec![],
            property_order: vec![],
            regular_fields: vec![],
            regular_members: vec![],
            initial_render_body: String::new(),
            initial_render_span_hints: vec![],
            builder_methods: vec![],
            styles_methods: vec![],
            entry_registration: None,
        };
        let out = generate_view_pu_class(&info);
        assert!(
            out.contains("new SynchedPropertySimpleOneWayPU(params.title, this, \"title\")"),
            "@Prop constructor must use params.name, even when the field has a local initializer"
        );
        assert!(
            !out.contains("new SynchedPropertySimpleOneWayPU(\"default\""),
            "@Prop must not use the local initializer in the constructor"
        );
    }

    // ── Storage decorators ─────────────────────────────────────────────────────

    #[test]
    fn test_storage_decorators_use_abstract_pu_backing_class() {
        for decorator in [
            "StorageProp",
            "StorageLink",
            "LocalStorageProp",
            "LocalStorageLink",
        ] {
            let info = ComponentInfo {
                name: "StorageComp".to_string(),
                is_entry: false,
                is_exported: false,
                is_default_export: false,
                is_reusable: false,
                state_fields: vec![StateField {
                    name: "setting".to_string(),
                    type_annotation: "string".to_string(),
                    decorator: decorator.to_string(),
                    initializer: None,
                    watch_handler: None,
                    alias: None,
                    decorator_arg: None,
                    is_simple_type: true,
                }],
                params_fields: vec![],
                property_order: vec![],
                regular_fields: vec![],
                initial_render_body: String::new(),
                initial_render_span_hints: vec![],
                regular_members: vec![],
                builder_methods: vec![],
                styles_methods: vec![],
                entry_registration: None,
            };
            let out = generate_view_pu_class(&info);
            assert!(
                out.contains("private __setting: ObservedPropertyAbstractPU<string>"),
                "wrong backing class for @{decorator}"
            );
            assert!(
                if decorator.starts_with("LocalStorage") {
                    out.contains(&format!(
                        "private __setting: ObservedPropertyAbstractPU<string> = this.{}<string>(\"setting\", undefined, \"setting\")",
                        if decorator.ends_with("Link") {
                            "createLocalStorageLink"
                        } else {
                            "createLocalStorageProp"
                        }
                    ))
                } else {
                    out.contains(&format!(
                        "this.__setting = this.{}(\"setting\", undefined, \"setting\")",
                        if decorator.ends_with("Link") {
                            "createStorageLink"
                        } else {
                            "createStorageProp"
                        }
                    ))
                },
                "@{decorator} must emit hvigor storage runtime init"
            );
        }
    }
}
