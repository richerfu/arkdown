use crate::transform::component::{RegularField, StateField};
use crate::transform::decorator::{
    constructor_init_kind, is_state_decorator, state_decorator_class_for,
    state_decorator_decl_class_for, ConstructorInitKind,
};

pub(super) fn push_state_constructor_init(out: &mut String, field: &StateField) {
    let backing = state_decorator_class_for(&field.decorator, field.is_simple_type);
    let local_init = field.initializer.as_deref().unwrap_or("undefined");
    match constructor_init_kind(&field.decorator) {
        ConstructorInitKind::NewWithLocalInit => {
            out.push_str(&format!(
                "        this.__{name} = new {backing}({init}, this, \"{name}\");\n",
                name = field.name,
                backing = backing,
                init = local_init,
            ));
        }
        ConstructorInitKind::NewWithParamInit => {
            out.push_str(&format!(
                "        this.__{name} = new {backing}(params.{name}, this, \"{name}\");\n",
                name = field.name,
                backing = backing,
            ));
        }
        ConstructorInitKind::ConsumeInit => {
            let key = field.alias.as_deref().unwrap_or(field.name.as_str());
            out.push_str(&format!(
                "        this.__{name} = this.initializeConsume(\"{key}\", \"{name}\");\n",
                name = field.name,
                key = key,
            ));
        }
        ConstructorInitKind::BuilderParamInit => {
            out.push_str(&format!(
                "        this.{name} = {init};\n",
                name = field.name,
                init = local_init,
            ));
        }
        ConstructorInitKind::StorageInit => {
            if matches!(
                field.decorator.as_str(),
                "LocalStorageLink" | "LocalStorageProp" | "LocalStoragePropRef"
            ) {
                return;
            }
            let key = storage_key_expr(field);
            let api = match field.decorator.as_str() {
                "StorageLink" | "LocalStorageLink" => "createStorageLink",
                _ => "createStorageProp",
            };
            out.push_str(&format!(
                "        this.__{name} = this.{api}({key}, {init}, \"{name}\");\n",
                name = field.name,
                api = api,
                key = key,
                init = local_init,
            ));
        }
    }
}

pub(super) fn push_regular_field_declaration(out: &mut String, field: &RegularField) {
    out.push_str("    private ");
    out.push_str(&field.name);
    if field.optional {
        out.push('?');
    }
    if !field.type_annotation.is_empty() {
        out.push_str(": ");
        out.push_str(&field.type_annotation);
    }
    out.push_str(";\n");
}

pub(super) fn push_state_field_declaration(out: &mut String, field: &StateField) {
    if field.decorator == "BuilderParam" {
        out.push_str(&format!("    private __{}?;\n", field.name));
        return;
    }

    let backing = state_decorator_decl_class_for(&field.decorator, field.is_simple_type);
    let type_param = state_type_param(field);
    out.push_str(&format!(
        "    private __{name}: {backing}{type_param}",
        name = field.name,
        backing = backing,
        type_param = type_param,
    ));
    if matches!(
        field.decorator.as_str(),
        "LocalStorageLink" | "LocalStorageProp" | "LocalStoragePropRef"
    ) {
        push_local_storage_initializer(out, field, &type_param);
    }
    out.push_str(";\n");
    if is_state_decorator(&field.decorator) {
        push_state_accessors(out, field);
    }
}

fn push_local_storage_initializer(out: &mut String, field: &StateField, type_param: &str) {
    let local_init = field.initializer.as_deref().unwrap_or("undefined");
    let key = storage_key_expr(field);
    let api = match field.decorator.as_str() {
        "LocalStorageLink" => "createLocalStorageLink",
        _ => "createLocalStorageProp",
    };
    out.push_str(&format!(
        " = this.{api}{type_param}({key}, {init}, \"{name}\")",
        name = field.name,
        api = api,
        type_param = type_param,
        key = key,
        init = local_init,
    ));
}

fn push_state_accessors(out: &mut String, field: &StateField) {
    let ty = if field.type_annotation.is_empty() {
        "unknown"
    } else {
        field.type_annotation.as_str()
    };
    out.push_str(&format!(
        "    get {name}() {{\n        return this.__{name}.get();\n    }}\n",
        name = field.name,
    ));
    out.push_str(&format!(
        "    set {name}(newValue: {ty}) {{\n        this.__{name}.set(newValue);\n    }}\n",
        name = field.name,
        ty = ty,
    ));
}

fn state_type_param(field: &StateField) -> String {
    if field.type_annotation.is_empty() {
        String::new()
    } else {
        format!("<{}>", field.type_annotation)
    }
}

fn storage_key_expr(field: &StateField) -> String {
    field
        .decorator_arg
        .as_deref()
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("\"{}\"", field.name))
}
