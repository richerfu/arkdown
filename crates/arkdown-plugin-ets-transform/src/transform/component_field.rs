use std::fmt::Write;

use crate::transform::component::{RegularField, StateField};
use crate::transform::decorator::{
    constructor_init_kind, is_state_decorator, state_decorator_class_for,
    state_decorator_decl_class_for, ConstructorInitKind,
};

pub(super) fn push_state_constructor_init(out: &mut String, field: &StateField) {
    ComponentFieldEmitter::new(out).push_state_constructor_init(field);
}

/// Emit the `@Prop` default-value fallback block in the constructor.
///
/// Upstream hvigor `createUpdateParamsWithIf(e, t=true, r=initializer)` in
/// `process_component_member.js` — only emitted when:
/// 1. The decorator is `@Prop` (partialUpdate mode).
/// 2. The field has an initializer expression.
///
/// Generated code:
/// ```typescript
/// if (params.fieldName === undefined) {
///     this.__fieldName.set(defaultValue);
/// }
/// ```
pub(super) fn push_prop_default_fallback(out: &mut String, field: &StateField) {
    if field.decorator != "Prop" {
        return;
    }
    let Some(init) = field.initializer.as_deref() else {
        return;
    };
    let _ = writeln!(
        out,
        "        if (params.{} === undefined) {{\n            this.__{}.set({});\n        }}",
        field.name, field.name, init
    );
}

/// Emit `this.addProvidedVar(...)` registration call(s) for a `@Provide` field.
///
/// Upstream hvigor `addAddProvidedVar` (process_component_member.js):
/// - No alias: one call with the field name.
/// - Alias != field name: two calls — alias first, then field name.
/// - Alias == field name: one call with that name.
pub(super) fn push_provide_var_registration(out: &mut String, field: &StateField) {
    if field.decorator != "Provide" {
        return;
    }
    let var_name = &field.name;
    if let Some(alias) = &field.alias {
        if alias != var_name {
            let _ = writeln!(
                out,
                "        this.addProvidedVar(\"{}\", this.__{}, false);",
                alias, var_name
            );
        }
    }
    let _ = writeln!(
        out,
        "        this.addProvidedVar(\"{}\", this.__{}, false);",
        var_name, var_name
    );
}

pub(super) fn push_regular_field_declaration(out: &mut String, field: &RegularField) {
    ComponentFieldEmitter::new(out).push_regular_field_declaration(field);
}

pub(super) fn push_state_field_declaration(out: &mut String, field: &StateField) {
    ComponentFieldEmitter::new(out).push_state_field_declaration(field);
}

struct ComponentFieldEmitter<'a> {
    out: &'a mut String,
}

impl<'a> ComponentFieldEmitter<'a> {
    fn new(out: &'a mut String) -> Self {
        Self { out }
    }

    fn push_state_constructor_init(&mut self, field: &StateField) {
        let backing = state_decorator_class_for(&field.decorator, field.is_simple_type);
        let local_init = field.initializer.as_deref().unwrap_or("undefined");
        match constructor_init_kind(&field.decorator) {
            ConstructorInitKind::NewWithLocalInit => {
                let _ = writeln!(
                    self.out,
                    "        this.__{} = new {}({}, this, \"{}\");",
                    field.name, backing, local_init, field.name
                );
            }
            ConstructorInitKind::NewWithParamInit => {
                let _ = writeln!(
                    self.out,
                    "        this.__{} = new {}(params.{}, this, \"{}\");",
                    field.name, backing, field.name, field.name
                );
            }
            ConstructorInitKind::ConsumeInit => {
                let key = field.alias.as_deref().unwrap_or(field.name.as_str());
                let _ = writeln!(
                    self.out,
                    "        this.__{} = this.initializeConsume(\"{}\", \"{}\");",
                    field.name, key, field.name
                );
            }
            ConstructorInitKind::BuilderParamInit => {
                let _ = writeln!(self.out, "        this.{} = {};", field.name, local_init);
            }
            ConstructorInitKind::StorageInit => {
                if field.uses_local_storage() {
                    return;
                }
                let _ = writeln!(
                    self.out,
                    "        this.__{} = this.{}({}, {}, \"{}\");",
                    field.name,
                    field.storage_api(),
                    field.storage_key_expr(),
                    local_init,
                    field.name
                );
            }
        }
    }

    fn push_regular_field_declaration(&mut self, field: &RegularField) {
        let _ = write!(self.out, "    private {}", field.name);
        if field.optional {
            self.out.push('?');
        }
        if !field.type_annotation.is_empty() {
            let _ = write!(self.out, ": {}", field.type_annotation);
        }
        self.out.push_str(";\n");
    }

    fn push_state_field_declaration(&mut self, field: &StateField) {
        if field.decorator == "BuilderParam" {
            let _ = writeln!(self.out, "    private __{}?;", field.name);
            return;
        }

        let backing = state_decorator_decl_class_for(&field.decorator, field.is_simple_type);
        let type_param = field.state_type_param();
        let _ = write!(
            self.out,
            "    private __{}: {}{}",
            field.name, backing, type_param
        );
        if field.uses_local_storage() {
            self.push_local_storage_initializer(field, &type_param);
        }
        self.out.push_str(";\n");
        if is_state_decorator(&field.decorator) {
            self.push_state_accessors(field);
        }
    }

    fn push_local_storage_initializer(&mut self, field: &StateField, type_param: &str) {
        let local_init = field.initializer.as_deref().unwrap_or("undefined");
        let _ = write!(
            self.out,
            " = this.{}{}({}, {}, \"{}\")",
            field.local_storage_api(),
            type_param,
            field.storage_key_expr(),
            local_init,
            field.name
        );
    }

    fn push_state_accessors(&mut self, field: &StateField) {
        let ty = if field.type_annotation.is_empty() {
            "unknown"
        } else {
            field.type_annotation.as_str()
        };
        let _ = writeln!(
            self.out,
            "    get {}() {{\n        return this.__{}.get();\n    }}",
            field.name, field.name
        );
        let _ = writeln!(
            self.out,
            "    set {}(newValue: {}) {{\n        this.__{}.set(newValue);\n    }}",
            field.name, ty, field.name
        );
    }
}

trait StateFieldEmitExt {
    fn state_type_param(&self) -> String;
    fn storage_key_expr(&self) -> String;
    fn storage_api(&self) -> &'static str;
    fn local_storage_api(&self) -> &'static str;
    fn uses_local_storage(&self) -> bool;
}

impl StateFieldEmitExt for StateField {
    fn state_type_param(&self) -> String {
        if self.type_annotation.is_empty() {
            String::new()
        } else {
            format!("<{}>", self.type_annotation)
        }
    }

    fn storage_key_expr(&self) -> String {
        self.decorator_arg
            .as_deref()
            .map(str::trim)
            .filter(|arg| !arg.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("\"{}\"", self.name))
    }

    fn storage_api(&self) -> &'static str {
        match self.decorator.as_str() {
            "StorageLink" | "LocalStorageLink" => "createStorageLink",
            _ => "createStorageProp",
        }
    }

    fn local_storage_api(&self) -> &'static str {
        match self.decorator.as_str() {
            "LocalStorageLink" => "createLocalStorageLink",
            _ => "createLocalStorageProp",
        }
    }

    fn uses_local_storage(&self) -> bool {
        matches!(
            self.decorator.as_str(),
            "LocalStorageLink" | "LocalStorageProp" | "LocalStoragePropRef"
        )
    }
}
