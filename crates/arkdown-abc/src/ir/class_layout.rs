use oxc_ast::ast::{ClassElement, MethodDefinition, MethodDefinitionKind, PropertyKey};

use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::ir::function_records::FunctionNameAllocator;

pub(super) fn class_method_record_name(
    method: &MethodDefinition<'_>,
    class_name: &str,
    function_names: &mut FunctionNameAllocator<'_>,
) -> Option<String> {
    let name = if method.kind == MethodDefinitionKind::Constructor {
        class_name.to_string()
    } else {
        method_key_name(&method.key)?
    };
    Some(function_names.bind(method.value.span.start, method.value.span.end, Some(&name)))
}

pub(super) fn class_method_num_args(method: &MethodDefinition<'_>) -> u16 {
    method.value.params.items.len() as u16 + 3
}

pub(super) fn method_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str().to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.as_str().to_string()),
        _ => None,
    }
}

pub(super) fn class_buffer_literal_array(
    _class_name: &str,
    elements: &[ClassElement<'_>],
) -> LiteralArray {
    let mut entries = Vec::new();
    let mut instance_method_count = 0u32;

    for element in elements {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };

        if !is_class_buffer_method(method) {
            continue;
        }
        let Some(public_name) = method_key_name(&method.key) else {
            continue;
        };

        entries.push(LiteralEntry::TaggedString(public_name.clone()));
        entries.push(LiteralEntry::TaggedMethodName(public_name));
        entries.push(LiteralEntry::TaggedMethodAffiliate(
            method.value.params.items.len() as u16,
        ));
        instance_method_count += 1;
    }

    entries.push(LiteralEntry::TaggedInteger(instance_method_count));
    LiteralArray { entries }
}

pub(super) fn is_class_buffer_method(method: &MethodDefinition<'_>) -> bool {
    if method.kind == MethodDefinitionKind::Constructor || method.kind.is_accessor() {
        return false;
    }
    if method.r#static {
        return false;
    }
    let Some(key) = method_key_name(&method.key) else {
        return false;
    };
    !matches!(key.as_str(), "initialRender" | "rerender")
}

pub(super) fn constructor_required_param_count(elements: &[ClassElement<'_>]) -> u16 {
    elements
        .iter()
        .find_map(|element| {
            let ClassElement::MethodDefinition(method) = element else {
                return None;
            };
            (method.kind == MethodDefinitionKind::Constructor).then(|| {
                method
                    .value
                    .params
                    .items
                    .iter()
                    .take_while(|param| param.initializer.is_none())
                    .count() as u16
            })
        })
        .unwrap_or(0)
}
