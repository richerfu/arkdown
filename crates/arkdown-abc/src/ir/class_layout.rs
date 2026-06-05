use oxc_ast::ast::{
    ClassElement, MethodDefinition, MethodDefinitionKind, PropertyDefinition, PropertyKey,
};
use oxc_span::GetSpan;

use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::ir::function_records::FunctionNameAllocator;

pub(super) fn class_method_record_name(
    method: &MethodDefinition<'_>,
    class_name: &str,
    function_names: &mut FunctionNameAllocator<'_>,
    standalone_ecmascript_input: bool,
) -> Option<String> {
    if standalone_ecmascript_input {
        return class_method_upstream_name(method, class_name);
    }
    let name = class_method_inferred_name(method, class_name)?;
    Some(function_names.bind(method.value.span.start, method.value.span.end, Some(&name)))
}

pub(super) fn class_method_upstream_name(
    method: &MethodDefinition<'_>,
    class_name: &str,
) -> Option<String> {
    if method.kind == MethodDefinitionKind::Constructor {
        return Some(format!("#~{class_name}=#{class_name}"));
    }
    let marker = if method.r#static { '<' } else { '>' };
    let key_name = method_key_name(&method.key).or_else(|| {
        method
            .computed
            .then(|| computed_class_method_record_name(method))
    })?;
    Some(format!("#~{class_name}{marker}#{key_name}"))
}

pub(super) fn class_method_inferred_name(
    method: &MethodDefinition<'_>,
    class_name: &str,
) -> Option<String> {
    if method.kind == MethodDefinitionKind::Constructor {
        return Some(class_name.to_string());
    }
    method_key_name(&method.key).or_else(|| {
        method
            .computed
            .then(|| computed_class_method_record_name(method))
    })
}

pub(super) fn computed_class_method_record_name(method: &MethodDefinition<'_>) -> String {
    format!(
        "#computed_class_method_{}_{}#",
        method.value.span.start, method.value.span.end
    )
}

pub(super) fn computed_instance_class_field_key_name(property: &PropertyDefinition<'_>) -> String {
    format!(
        "#computed_instance_class_field_key_{}_{}#",
        property.key.span().start,
        property.key.span().end
    )
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
    class_name: &str,
    elements: &[ClassElement<'_>],
    standalone_ecmascript_input: bool,
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
        let method_name = if standalone_ecmascript_input {
            class_method_upstream_name(method, class_name).unwrap_or_else(|| public_name.clone())
        } else {
            public_name.clone()
        };

        entries.push(LiteralEntry::TaggedString(public_name.clone()));
        entries.push(LiteralEntry::TaggedMethodName(method_name));
        entries.push(LiteralEntry::TaggedMethodAffiliate(
            method.value.params.items.len() as u16,
        ));
        instance_method_count += 1;
    }

    entries.push(LiteralEntry::TaggedInteger(instance_method_count));
    LiteralArray {
        entries,
        explicit_key: None,
    }
}

/// Build the private-methods literal array for `callruntime.createprivateproperty`.
///
/// Mirrors upstream `ClassDefinition::CreateClassPrivateBuffer`:
/// - For each private METHOD (not field), adds a method name entry and a
///   method-affiliate (param count) entry.
/// - Ends with a count of instance private methods.
/// Static private method entries come first, then instance entries, then the count.
///
/// For classes with only private FIELDS (no private methods), returns an array
/// with just the instance private method count (= 0).
pub(super) fn class_private_methods_literal_array(elements: &[ClassElement<'_>]) -> LiteralArray {
    let mut instance_entries: Vec<LiteralEntry> = Vec::new();
    let mut static_entries: Vec<LiteralEntry> = Vec::new();
    let mut instance_method_count = 0u32;

    for element in elements {
        let ClassElement::MethodDefinition(method) = element else {
            continue;
        };
        // Only private methods (not private fields).
        let PropertyKey::PrivateIdentifier(id) = &method.key else {
            continue;
        };
        if method.kind == MethodDefinitionKind::Constructor {
            continue;
        }
        let key_name = id.name.as_str().to_string();
        let affiliate = LiteralEntry::TaggedMethodAffiliate(method.value.params.items.len() as u16);
        if method.r#static {
            static_entries.push(LiteralEntry::TaggedMethodName(key_name));
            static_entries.push(affiliate);
        } else {
            instance_entries.push(LiteralEntry::TaggedMethodName(key_name));
            instance_entries.push(affiliate);
            instance_method_count += 1;
        }
    }

    // Static entries come first, then instance entries, then the count.
    let mut entries = static_entries;
    entries.extend(instance_entries);
    entries.push(LiteralEntry::TaggedInteger(instance_method_count));
    LiteralArray {
        entries,
        explicit_key: None,
    }
}

pub(super) fn static_initializer_record_name(
    class_name: &str,
    elements: &[ClassElement<'_>],
) -> Option<String> {
    let first_initializer_start = elements
        .iter()
        .filter_map(|element| match element {
            ClassElement::PropertyDefinition(property)
                if property.r#static
                    && (property.computed || method_key_name(&property.key).is_some()) =>
            {
                Some(property.key.span().start)
            }
            ClassElement::StaticBlock(block) => Some(block.span.start),
            _ => None,
        })
        .min()?;
    Some(format!(
        "#{class_name}_{first_initializer_start}_static_initializer#"
    ))
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
