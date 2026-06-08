use oxc_ast::ast::{
    AccessorProperty, ClassElement, MethodDefinition, MethodDefinitionKind, PropertyDefinition,
    PropertyKey,
};
use rustc_hash::FxHashMap;

use crate::emit::file::{LiteralArray, LiteralEntry};
use crate::ir::function_records::FunctionNameAllocator;
use crate::ir::lowering_context::LoweringContext;

pub(super) const DEFAULT_USE_DEFINE_SEMANTIC: bool = false;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ClassStaticInitializationMode {
    ExternalInitializerMethod,
    InlineInModuleMain,
}

impl ClassStaticInitializationMode {
    pub(super) const fn uses_external_initializer_method(self) -> bool {
        matches!(self, Self::ExternalInitializerMethod)
    }

    pub(super) const fn inlines_in_module_main(self) -> bool {
        matches!(self, Self::InlineInModuleMain)
    }
}

impl Default for ClassStaticInitializationMode {
    fn default() -> Self {
        Self::ExternalInitializerMethod
    }
}

pub(super) fn property_needs_instance_initializer(
    property: &PropertyDefinition<'_>,
    use_define_semantic: bool,
) -> bool {
    if property.r#static {
        return false;
    }
    if matches!(property.key, PropertyKey::PrivateIdentifier(_)) {
        return true;
    }
    use_define_semantic
}

pub(super) fn property_needs_constructor_assignment(
    property: &PropertyDefinition<'_>,
    use_define_semantic: bool,
) -> bool {
    !use_define_semantic
        && !property.r#static
        && !matches!(property.key, PropertyKey::PrivateIdentifier(_))
        && property.value.is_some()
}

pub(super) fn accessor_needs_instance_initializer(accessor: &AccessorProperty<'_>) -> bool {
    !accessor.r#static
}

pub(super) fn property_needs_static_initializer(property: &PropertyDefinition<'_>) -> bool {
    property.r#static
        && (property.computed
            || method_key_name(&property.key).is_some()
            || matches!(property.key, PropertyKey::PrivateIdentifier(_)))
}

pub(super) fn class_needs_static_initializer(elements: &[ClassElement<'_>]) -> bool {
    elements.iter().any(|element| match element {
        ClassElement::PropertyDefinition(property) => property_needs_static_initializer(property),
        ClassElement::StaticBlock(_) => true,
        _ => false,
    })
}

pub(super) fn method_is_instance_private(method: &MethodDefinition<'_>) -> bool {
    !method.r#static && matches!(method.key, PropertyKey::PrivateIdentifier(_))
}

pub(super) fn class_needs_instance_initializer(
    elements: &[ClassElement<'_>],
    use_define_semantic: bool,
) -> bool {
    elements.iter().any(|element| match element {
        ClassElement::PropertyDefinition(property) => {
            property_needs_instance_initializer(property, use_define_semantic)
        }
        ClassElement::AccessorProperty(accessor) => accessor_needs_instance_initializer(accessor),
        ClassElement::MethodDefinition(method) => method_is_instance_private(method),
        _ => false,
    })
}

pub(super) fn class_has_constructor(elements: &[ClassElement<'_>]) -> bool {
    elements.iter().any(|element| {
        matches!(
            element,
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor
        )
    })
}

pub(super) fn class_requires_external_instance_initializer(
    elements: &[ClassElement<'_>],
    has_super_class: bool,
    standalone_ecmascript_input: bool,
    use_define_semantic: bool,
) -> bool {
    class_needs_instance_initializer(elements, use_define_semantic)
        && !class_has_constructor(elements)
        && (has_super_class || standalone_ecmascript_input)
}

pub(super) fn class_method_record_name(
    method: &MethodDefinition<'_>,
    class_name: &str,
    class_span_start: u32,
    class_span_end: u32,
    function_names: &mut FunctionNameAllocator<'_>,
    standalone_ecmascript_input: bool,
) -> Option<String> {
    if let Some(name) =
        function_names.class_method_name(method.value.span.start, method.value.span.end)
    {
        return Some(name.to_string());
    }
    if function_names.uses_scope_info_names() {
        return function_names.bind_class_method(
            (class_span_start, class_span_end),
            method,
            class_name,
        );
    }
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

pub(super) struct ClassPublicBuffer {
    pub literal_array: LiteralArray,
    compiled_methods: Vec<bool>,
}

impl ClassPublicBuffer {
    pub(super) fn build(
        class_name: &str,
        elements: &[ClassElement<'_>],
        standalone_ecmascript_input: bool,
        lowering: Option<&LoweringContext>,
    ) -> Self {
        let mut instance_entries = Vec::new();
        let mut static_entries = Vec::new();
        let mut instance_method_count = 0u32;
        let mut compiled_methods = vec![false; elements.len()];
        let mut instance_positions = FxHashMap::default();
        let mut static_positions = FxHashMap::default();

        for (index, element) in elements.iter().enumerate() {
            let ClassElement::MethodDefinition(method) = element else {
                continue;
            };

            if method.kind == MethodDefinitionKind::Constructor {
                continue;
            }
            if matches!(method.key, PropertyKey::PrivateIdentifier(_)) {
                continue;
            }
            if method.computed || method.kind.is_accessor() {
                break;
            }

            let Some(public_name) = method_key_name(&method.key) else {
                continue;
            };
            let method_name = if let Some(name) = lowering.and_then(|lowering| {
                lowering.function_name(method.value.span.start, method.value.span.end)
            }) {
                name.to_string()
            } else if standalone_ecmascript_input {
                class_method_upstream_name(method, class_name)
                    .unwrap_or_else(|| public_name.clone())
            } else {
                public_name.clone()
            };

            let (entries, positions) = if method.r#static {
                (&mut static_entries, &mut static_positions)
            } else {
                (&mut instance_entries, &mut instance_positions)
            };
            let buffer_pos = if let Some(pos) = positions.get(&public_name).copied() {
                pos
            } else {
                let pos = entries.len();
                positions.insert(public_name.clone(), pos);
                if !method.r#static {
                    instance_method_count += 1;
                }
                entries.push(LiteralEntry::TaggedString(public_name));
                entries.push(LiteralEntry::TaggedNull);
                entries.push(LiteralEntry::TaggedNull);
                pos
            };

            entries[buffer_pos + 1] = LiteralEntry::TaggedMethodName(method_name);
            entries[buffer_pos + 2] =
                LiteralEntry::TaggedMethodAffiliate(method.value.params.items.len() as u16);
            compiled_methods[index] = true;
        }

        instance_entries.extend(static_entries);
        instance_entries.push(LiteralEntry::TaggedInteger(instance_method_count));
        Self {
            literal_array: LiteralArray {
                entries: instance_entries,
                explicit_key: None,
            },
            compiled_methods,
        }
    }

    pub(super) fn into_parts(self) -> (LiteralArray, Vec<bool>) {
        (self.literal_array, self.compiled_methods)
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
    class_span_start: u32,
    class_span_end: u32,
    elements: &[ClassElement<'_>],
    lowering: Option<&LoweringContext>,
) -> Option<String> {
    if !class_needs_static_initializer(elements) {
        return None;
    }
    if lowering.is_some_and(|lowering| {
        lowering
            .static_initialization_mode()
            .inlines_in_module_main()
    }) {
        return None;
    }
    lowering
        .and_then(|lowering| lowering.static_initializer_name(class_span_start, class_span_end))
        .map(str::to_string)
        .or_else(|| Some(format!("#~{class_name}>#static_initializer")))
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
