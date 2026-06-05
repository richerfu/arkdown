use std::collections::{BTreeMap, BTreeSet};

use rustc_hash::FxHashMap;

use crate::abc_reader::{AbcClassItem, AbcFieldValue, AbcLiteralEntry, AbcReader};
use crate::emit::bytecode::instruction_len;
use crate::emit::file::{
    AbcFile, LiteralArray, LiteralEntry, LITERAL_TAG_ACCESSOR, LITERAL_TAG_ASYNC_GENERATOR_METHOD,
    LITERAL_TAG_BUILTIN_TYPE_INDEX, LITERAL_TAG_ETS_IMPLEMENTS, LITERAL_TAG_GENERATOR_METHOD,
    LITERAL_TAG_GETTER, LITERAL_TAG_INTEGER, LITERAL_TAG_LITERAL_BUFFER_INDEX, LITERAL_TAG_METHOD,
    LITERAL_TAG_METHOD_AFFILIATE, LITERAL_TAG_SETTER, LITERAL_TAG_STRING,
};
use crate::emit::record::{
    function_kind, method_access, AnnotationElementRecord, AnnotationRecord, AnnotationValue,
    ArrayValueRecord, CatchEntry, ClassRecord, FieldRecord, FieldType, FieldValue, MethodKind,
    MethodLabelStyle, MethodRecord, ScalarValueData, ScalarValueRecord, FIELD_OWNER_CLASS_IDX,
    VALUE_TYPE_ANNOTATION, VALUE_TYPE_ENUM, VALUE_TYPE_F32, VALUE_TYPE_F64, VALUE_TYPE_I16,
    VALUE_TYPE_I32, VALUE_TYPE_I64, VALUE_TYPE_I8, VALUE_TYPE_LITERALARRAY, VALUE_TYPE_METHOD,
    VALUE_TYPE_METHOD_HANDLE, VALUE_TYPE_RECORD, VALUE_TYPE_STRING, VALUE_TYPE_STRING_NULLPTR,
    VALUE_TYPE_U1, VALUE_TYPE_U16, VALUE_TYPE_U32, VALUE_TYPE_U64, VALUE_TYPE_U8,
};
use crate::error::AbcError;
use crate::ir::opcodes::{Insn, PandasmInsn};
use crate::isa::{
    decode_pandasm_instruction, first_id_operand as isa_first_id_operand,
    id_operands as isa_id_operands, instruction_len as isa_instruction_len,
    is_call_instruction as isa_is_call_instruction, IdKind,
};
use crate::names::{full_method_name, literal_array_key};

pub(crate) fn append_abc_file(file: &mut AbcFile, data: &[u8]) -> Result<Vec<usize>, AbcError> {
    let reader = AbcReader::new(data).map_err(read_error("<abc input>"))?;
    let classes = reader.class_items().map_err(read_error("<abc input>"))?;

    let module_literal_offsets = classes
        .iter()
        .flat_map(|class| {
            class.fields.iter().filter_map(|field| {
                (field.name == "moduleRecordIdx")
                    .then_some(field.value)
                    .flatten()
            })
        })
        .filter_map(|value| match value {
            AbcFieldValue::U32(offset) => Some(offset),
            AbcFieldValue::U8(_) => None,
        })
        .collect::<BTreeSet<_>>();

    let literal_offsets = reader
        .literal_array_offsets()
        .map_err(read_error("<abc input>"))?;
    let literal_base_index = file.literal_arrays.len();
    let literal_offset_to_index = literal_offsets
        .iter()
        .enumerate()
        .map(|(index, &offset)| (offset, literal_base_index + index))
        .collect::<BTreeMap<_, _>>();
    for offset in literal_offsets {
        let literal = if module_literal_offsets.contains(&offset) {
            read_module_record_literal_array(data, &reader, offset)?
        } else {
            read_tagged_literal_array(&reader, offset, &literal_offset_to_index)?
        };
        file.literal_arrays.push(literal);
    }

    let method_names = classes
        .iter()
        .flat_map(|class| {
            class
                .methods
                .iter()
                .map(|method| (method.offset, method.name.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let index_sections = reader.index_sections().map_err(read_error("<abc input>"))?;
    let class_name_by_offset = classes
        .iter()
        .map(|class| (class.offset, class.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let method_pool_offsets = index_sections
        .iter()
        .flat_map(|section| section.method_offsets.iter().copied())
        .filter(|offset| method_names.contains_key(offset))
        .collect::<BTreeSet<_>>();
    let method_call_infos = build_method_call_infos(&reader, &classes)?;

    let mut appended = Vec::new();
    for class in classes {
        if is_ark_annotation_stub(&class.name) {
            continue;
        }

        let mut record_string_indices = BTreeSet::new();
        let mut record_literal_indices = BTreeSet::new();
        let fields = class
            .fields
            .iter()
            .map(|field| {
                let annotations = custom_annotations_from_abc(
                    data,
                    &reader,
                    &class,
                    field.offset,
                    &field.annotation_offsets,
                    &index_sections,
                    &class_name_by_offset,
                    &literal_offset_to_index,
                )?;
                let field_type =
                    field_type_from_abc(field, &index_sections, &class_name_by_offset)?;
                Ok(FieldRecord {
                    name: field.name.clone(),
                    class_idx: FIELD_OWNER_CLASS_IDX,
                    type_idx: field.type_idx,
                    field_type,
                    metadata_field_type: None,
                    access_flags: field.access_flags,
                    value: field_value_from_abc(
                        field.name.as_str(),
                        field.value,
                        &literal_offset_to_index,
                    )?,
                    annotations,
                    metadata_attrs: Default::default(),
                    source: Default::default(),
                })
            })
            .collect::<Result<Vec<_>, AbcError>>()?;

        let methods = class
            .methods
            .iter()
            .map(|method| {
                let code = method
                    .code
                    .as_ref()
                    .ok_or_else(|| AbcError::UnsupportedConstruct {
                        construct: format!("ABC method {} has no CodeItem", method.name),
                        file: class.name.clone(),
                    })?;
                let bytecode = unpatch_bytecode_index_operands(
                    &reader,
                    file,
                    method.offset,
                    &code.bytecode,
                    &literal_offset_to_index,
                    &method_names,
                    &mut record_string_indices,
                    &mut record_literal_indices,
                )?;
                let pandasm_insns = decode_bytecode(
                    &reader,
                    &bytecode,
                    &code.bytecode,
                    file,
                    class.name.as_str(),
                    method.offset,
                    &method_call_infos,
                )?;
                let kind = method_kind_from_access_flags(method.name.as_str(), method.access_flags);
                let annotations = method_annotations_from_abc(
                    data,
                    &reader,
                    &class,
                    method,
                    &index_sections,
                    &class_name_by_offset,
                    &literal_offset_to_index,
                )?;
                Ok(MethodRecord {
                    name: method.name.clone(),
                    bytecode,
                    pandasm_insns,
                    pandasm_debug: Vec::new(),
                    num_vregs: code.num_vregs as u16,
                    num_args: code.num_args as u16,
                    value_of_first_param: -1,
                    slots_num: annotations.slot_number,
                    debug_info: None,
                    line_ops: Vec::new(),
                    catch_entries: code
                        .catches
                        .iter()
                        .map(|catch| CatchEntry {
                            try_begin: catch.try_begin,
                            try_end: catch.try_end,
                            catch_begin: catch.handler_pc,
                            catch_end: catch.handler_pc.saturating_add(catch.code_size),
                        })
                        .collect(),
                    program_labels: Vec::new(),
                    label_table: Vec::new(),
                    kind,
                    function_kind: abc_default_function_kind(kind),
                    concurrent_module_requests: Vec::new(),
                    method_pool_entry: method_pool_offsets.contains(&method.offset),
                    expected_property_count: annotations.expected_property_count,
                    annotations: annotations.custom,
                    metadata_attrs: Default::default(),
                    param_annotations: method_param_annotations_from_abc(
                        data,
                        &reader,
                        &class,
                        method,
                        &index_sections,
                        &class_name_by_offset,
                        &literal_offset_to_index,
                    )?,
                    definefunc_order: None,
                    label_style: MethodLabelStyle::Abc2Program,
                    param_types: Vec::new(),
                    return_type: None,
                    body_presence: None,
                    body_location: None,
                    file_location: None,
                })
            })
            .collect::<Result<Vec<_>, AbcError>>()?;

        let record_index = file.records.len();
        let class_annotations = custom_annotations_from_abc(
            data,
            &reader,
            &class,
            class.offset,
            &class.annotation_offsets,
            &index_sections,
            &class_name_by_offset,
            &literal_offset_to_index,
        )?;
        file.add_record(ClassRecord {
            name: class.name,
            methods,
            string_indices: record_string_indices.into_iter().collect(),
            literal_indices: record_literal_indices.into_iter().collect(),
            module_literal_indices: class
                .fields
                .iter()
                .filter_map(|field| match (field.name.as_str(), field.value) {
                    ("moduleRecordIdx" | "scopeNames", Some(AbcFieldValue::U32(offset))) => {
                        literal_offset_to_index.get(&offset).copied()
                    }
                    _ => None,
                })
                .collect(),
            source_code_strings: Vec::new(),
            access_flags: class.access_flags,
            source_lang: class.source_lang.unwrap_or(0),
            metadata_attrs: Default::default(),
            conflict: false,
            params_num: 0,
            body_presence: false,
            file_location: None,
            standalone_ecmascript_input: false,
            source_file: class.source_file,
            annotations: class_annotations,
            module_requests: Vec::new(),
            fields,
        });
        appended.push(record_index);
    }

    Ok(appended)
}

fn read_error(file: &'static str) -> impl Fn(crate::abc_reader::AbcReadError) -> AbcError {
    move |error| AbcError::UnsupportedConstruct {
        construct: format!("ABC input parse failed: {error}"),
        file: file.to_string(),
    }
}

fn is_ark_annotation_stub(name: &str) -> bool {
    matches!(
        name,
        "L_ESConcurrentModuleRequestsAnnotation;"
            | "L_ESExpectedPropertyCountAnnotation;"
            | "L_ESSlotNumberAnnotation;"
    )
}

#[derive(Default)]
struct AbcDecodedMethodAnnotations {
    expected_property_count: Option<u32>,
    slot_number: Option<u32>,
    custom: Vec<AnnotationRecord>,
}

#[allow(clippy::too_many_arguments)]
fn custom_annotations_from_abc(
    data: &[u8],
    reader: &AbcReader<'_>,
    owner_class: &AbcClassItem,
    owner_offset: u32,
    annotation_offsets: &[u32],
    index_sections: &[crate::abc_reader::AbcIndexSection],
    class_name_by_offset: &BTreeMap<u32, String>,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<Vec<AnnotationRecord>, AbcError> {
    let Some(section) = index_sections
        .iter()
        .find(|section| section.start <= owner_offset && owner_offset < section.end)
        .or_else(|| index_sections.first())
    else {
        return Ok(Vec::new());
    };

    let mut annotations = Vec::new();
    for &annotation_off in annotation_offsets {
        let Some((record_name, elements)) = annotation_from_abc(
            data,
            reader,
            owner_class,
            annotation_off,
            section,
            class_name_by_offset,
            literal_offset_to_index,
        )?
        else {
            continue;
        };
        if is_system_function_annotation(record_name.as_bytes())
            || is_system_record_annotation(&record_name)
        {
            continue;
        }
        annotations.push(AnnotationRecord {
            record_name,
            elements,
        });
    }
    Ok(annotations)
}

fn method_annotations_from_abc(
    data: &[u8],
    reader: &AbcReader<'_>,
    owner_class: &AbcClassItem,
    method: &crate::abc_reader::AbcMethodItem,
    index_sections: &[crate::abc_reader::AbcIndexSection],
    class_name_by_offset: &BTreeMap<u32, String>,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<AbcDecodedMethodAnnotations, AbcError> {
    let Some(section) = index_sections
        .iter()
        .find(|section| section.start <= method.offset && method.offset < section.end)
        .or_else(|| index_sections.first())
    else {
        return Ok(AbcDecodedMethodAnnotations::default());
    };

    let mut decoded = AbcDecodedMethodAnnotations::default();
    for &annotation_off in &method.annotation_offsets {
        let Some((record_name, elements)) = annotation_from_abc(
            data,
            reader,
            owner_class,
            annotation_off,
            section,
            class_name_by_offset,
            literal_offset_to_index,
        )?
        else {
            continue;
        };
        if record_name == "_ESExpectedPropertyCountAnnotation" {
            decoded.expected_property_count = elements.iter().find_map(|element| {
                (element.name == "ExpectedPropertyCount").then_some(match &element.value {
                    AnnotationValue::U32(value) => Some(*value),
                    _ => None,
                })?
            });
            continue;
        }
        if record_name == "_ESSlotNumberAnnotation" {
            decoded.slot_number = elements.iter().find_map(|element| {
                (element.name == "SlotNumber").then_some(match &element.value {
                    AnnotationValue::U32(value) => Some(*value),
                    _ => None,
                })?
            });
            continue;
        }
        if record_name == "_ESConcurrentModuleRequestsAnnotation" {
            continue;
        }
        decoded.custom.push(AnnotationRecord {
            record_name,
            elements,
        });
    }
    Ok(decoded)
}

fn method_param_annotations_from_abc(
    data: &[u8],
    reader: &AbcReader<'_>,
    owner_class: &AbcClassItem,
    method: &crate::abc_reader::AbcMethodItem,
    index_sections: &[crate::abc_reader::AbcIndexSection],
    class_name_by_offset: &BTreeMap<u32, String>,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<Vec<Vec<AnnotationRecord>>, AbcError> {
    let Some(param_annotation_off) = method.param_annotation_offset else {
        return Ok(Vec::new());
    };
    let Some(section) = index_sections
        .iter()
        .find(|section| section.start <= method.offset && method.offset < section.end)
        .or_else(|| index_sections.first())
    else {
        return Ok(Vec::new());
    };

    let mut cursor = param_annotation_off as usize;
    let param_count = read_at_u32(data, cursor, "parameter annotation count")? as usize;
    cursor += 4;
    let mut params = Vec::with_capacity(param_count);
    for _ in 0..param_count {
        let annotation_count =
            read_at_u32(data, cursor, "parameter annotation item count")? as usize;
        cursor += 4;
        let mut annotations = Vec::with_capacity(annotation_count);
        for _ in 0..annotation_count {
            let annotation_off = read_at_u32(data, cursor, "parameter annotation offset")?;
            cursor += 4;
            let Some((record_name, elements)) = annotation_from_abc(
                data,
                reader,
                owner_class,
                annotation_off,
                section,
                class_name_by_offset,
                literal_offset_to_index,
            )?
            else {
                continue;
            };
            if is_system_function_annotation(record_name.as_bytes())
                || is_system_record_annotation(&record_name)
            {
                continue;
            }
            annotations.push(AnnotationRecord {
                record_name,
                elements,
            });
        }
        params.push(annotations);
    }
    Ok(params)
}

fn annotation_from_abc(
    data: &[u8],
    reader: &AbcReader<'_>,
    owner_class: &AbcClassItem,
    annotation_off: u32,
    section: &crate::abc_reader::AbcIndexSection,
    class_name_by_offset: &BTreeMap<u32, String>,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<Option<(String, Vec<AnnotationElementRecord>)>, AbcError> {
    let mut cursor = annotation_off as usize;
    let class_idx = read_at_u16(data, cursor, "annotation class_idx")?;
    cursor += 2;
    let Some(class_off) = section.class_offsets.get(class_idx as usize) else {
        return Err(AbcError::UnsupportedConstruct {
            construct: format!("ABC annotation class_idx {class_idx} out of IndexSection range"),
            file: owner_class.name.clone(),
        });
    };
    let Some(class_name) = class_name_by_offset.get(class_off) else {
        return Err(AbcError::UnsupportedConstruct {
            construct: format!("ABC annotation class offset {class_off:#x} missing"),
            file: owner_class.name.clone(),
        });
    };
    let Some(record_name) = annotation_record_name_from_class_name(class_name) else {
        return Ok(None);
    };
    if is_system_record_annotation(&record_name) {
        return Ok(None);
    }

    let count = read_at_u16(data, cursor, "annotation element count")? as usize;
    cursor += 2;
    let mut raw_elements = Vec::with_capacity(count);
    for _ in 0..count {
        let name_off = read_at_u32(data, cursor, "annotation element name")?;
        cursor += 4;
        let value = read_at_u32(data, cursor, "annotation element value")?;
        cursor += 4;
        raw_elements.push((name_off, value));
    }

    let mut elements = Vec::with_capacity(count);
    for (element_idx, (name_off, value)) in raw_elements.into_iter().enumerate() {
        let tag =
            *data
                .get(cursor + element_idx)
                .ok_or_else(|| AbcError::UnsupportedConstruct {
                    construct: "ABC annotation tag out of bounds".to_string(),
                    file: owner_class.name.clone(),
                })?;
        let name = reader
            .string_at(name_off)
            .map_err(read_error("<abc annotation>"))?;
        let value = annotation_value_from_abc(
            data,
            reader,
            owner_class,
            section,
            class_name_by_offset,
            tag,
            value,
            literal_offset_to_index,
        )?;
        elements.push(AnnotationElementRecord { name, value });
    }
    Ok(Some((record_name, elements)))
}

fn annotation_record_name_from_class_name(class_name: &str) -> Option<String> {
    class_name
        .strip_prefix('L')
        .and_then(|name| name.strip_suffix(';'))
        .map(str::to_string)
        .or_else(|| is_ark_annotation_stub(class_name).then(|| class_name.to_string()))
}

fn is_system_function_annotation(record_name: &[u8]) -> bool {
    record_name == b"_ESConcurrentModuleRequestsAnnotation"
        || record_name == b"_ESExpectedPropertyCountAnnotation"
        || record_name == b"_ESSlotNumberAnnotation"
}

fn is_system_record_annotation(record_name: &str) -> bool {
    matches!(
        record_name,
        "std.annotations.Module"
            | "std/annotations/Module"
            | "std.annotations.FunctionalReference"
            | "std/annotations/FunctionalReference"
            | "std.annotations.Async"
            | "std/annotations/Async"
    )
}

fn field_type_from_abc(
    field: &crate::abc_reader::AbcFieldItem,
    index_sections: &[crate::abc_reader::AbcIndexSection],
    class_name_by_offset: &BTreeMap<u32, String>,
) -> Result<FieldType, AbcError> {
    let Some(section) = index_sections
        .iter()
        .find(|section| section.start <= field.offset && field.offset < section.end)
        .or_else(|| index_sections.first())
    else {
        return Err(AbcError::UnsupportedConstruct {
            construct: "ABC field type index without IndexSection".to_string(),
            file: field.name.clone(),
        });
    };
    let Some(&entity_offset) = section.class_offsets.get(field.type_idx as usize) else {
        return Err(AbcError::UnsupportedConstruct {
            construct: format!(
                "ABC field type_idx {} missing from IndexSection",
                field.type_idx
            ),
            file: field.name.clone(),
        });
    };
    Ok(field_type_from_index_entity(
        entity_offset,
        class_name_by_offset,
    ))
}

fn field_type_from_index_entity(
    entity_offset: u32,
    class_name_by_offset: &BTreeMap<u32, String>,
) -> FieldType {
    match entity_offset {
        0 => FieldType::U1,
        1 => FieldType::I8,
        2 => FieldType::U8,
        3 => FieldType::I16,
        4 => FieldType::U16,
        5 => FieldType::I32,
        6 => FieldType::U32,
        7 => FieldType::F32,
        8 => FieldType::F64,
        9 => FieldType::I64,
        10 => FieldType::U64,
        12 => FieldType::Any,
        _ => class_name_by_offset
            .get(&entity_offset)
            .map(|descriptor| field_type_from_descriptor(descriptor))
            .unwrap_or(FieldType::Any),
    }
}

fn field_type_from_descriptor(descriptor: &str) -> FieldType {
    if descriptor == "Lpanda.String;" {
        return FieldType::PandaString;
    }

    let rank = descriptor.bytes().take_while(|&byte| byte == b'[').count();
    if rank == 0 {
        return FieldType::Any;
    }
    let component_descriptor = &descriptor[rank..];
    let component_name = match component_descriptor {
        "Z" => "u1".to_string(),
        "B" => "i8".to_string(),
        "H" => "u8".to_string(),
        "S" => "i16".to_string(),
        "C" => "u16".to_string(),
        "I" => "i32".to_string(),
        "U" => "u32".to_string(),
        "F" => "f32".to_string(),
        "D" => "f64".to_string(),
        "J" => "i64".to_string(),
        "Q" => "u64".to_string(),
        value if value.starts_with('L') && value.ends_with(';') => {
            value[1..value.len() - 1].replace('/', ".")
        }
        _ => "any".to_string(),
    };
    FieldType::Array {
        component_name,
        rank: rank as u32,
    }
}

fn annotation_value_from_abc(
    data: &[u8],
    reader: &AbcReader<'_>,
    owner_class: &AbcClassItem,
    section: &crate::abc_reader::AbcIndexSection,
    class_name_by_offset: &BTreeMap<u32, String>,
    tag: u8,
    value: u32,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<AnnotationValue, AbcError> {
    match tag {
        b'1' => Ok(AnnotationValue::Bool(value != 0)),
        b'7' => Ok(AnnotationValue::U32(value)),
        b'B' => Ok(AnnotationValue::F64(f64::from_bits(read_at_u64(
            data,
            value as usize,
            "annotation f64 value",
        )?))),
        b'C' => Ok(AnnotationValue::String(
            reader
                .string_at(value)
                .map_err(read_error("<abc annotation>"))?,
        )),
        b'#' => {
            let Some(&index) = literal_offset_to_index.get(&value) else {
                return Err(AbcError::UnsupportedConstruct {
                    construct: format!("ABC annotation literal array offset {value:#x} missing"),
                    file: owner_class.name.clone(),
                });
            };
            Ok(AnnotationValue::LiteralArray {
                index,
                key: literal_array_key(&owner_class.name, index),
            })
        }
        b'2' => Ok(raw_u64_annotation_value(VALUE_TYPE_I8, value as u64)),
        b'3' => Ok(raw_u64_annotation_value(VALUE_TYPE_U8, value as u64)),
        b'4' => Ok(raw_u64_annotation_value(VALUE_TYPE_I16, value as u64)),
        b'5' => Ok(raw_u64_annotation_value(VALUE_TYPE_U16, value as u64)),
        b'6' => Ok(raw_u64_annotation_value(VALUE_TYPE_I32, value as u64)),
        b'8' => Ok(raw_u64_annotation_value(
            VALUE_TYPE_I64,
            read_at_u64(data, value as usize, "annotation i64 value")?,
        )),
        b'9' => Ok(raw_u64_annotation_value(
            VALUE_TYPE_U64,
            read_at_u64(data, value as usize, "annotation u64 value")?,
        )),
        b'A' => Ok(AnnotationValue::RawScalar(ScalarValueRecord {
            value_type: VALUE_TYPE_F32,
            value: ScalarValueData::F32(f32::from_bits(value)),
        })),
        b'G' => {
            let Some((record_name, elements)) = annotation_from_abc(
                data,
                reader,
                owner_class,
                value,
                section,
                class_name_by_offset,
                literal_offset_to_index,
            )?
            else {
                return Ok(raw_u64_annotation_value(
                    VALUE_TYPE_ANNOTATION,
                    value as u64,
                ));
            };
            Ok(AnnotationValue::RawScalar(ScalarValueRecord {
                value_type: VALUE_TYPE_ANNOTATION,
                value: ScalarValueData::Annotation(Box::new(AnnotationRecord {
                    record_name,
                    elements,
                })),
            }))
        }
        b'K'..=b'Z' | b'@' | b'H' => Ok(AnnotationValue::Array(array_value_from_abc(
            data,
            reader,
            owner_class,
            section,
            class_name_by_offset,
            tag,
            value,
            literal_offset_to_index,
        )?)),
        other => Ok(AnnotationValue::RawScalar(ScalarValueRecord {
            value_type: annotation_tag_value_type(other),
            value: ScalarValueData::U64(value as u64),
        })),
    }
}

fn raw_u64_annotation_value(value_type: u32, value: u64) -> AnnotationValue {
    AnnotationValue::RawScalar(ScalarValueRecord {
        value_type,
        value: ScalarValueData::U64(value),
    })
}

fn array_value_from_abc(
    data: &[u8],
    reader: &AbcReader<'_>,
    owner_class: &AbcClassItem,
    section: &crate::abc_reader::AbcIndexSection,
    class_name_by_offset: &BTreeMap<u32, String>,
    tag: u8,
    value: u32,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<ArrayValueRecord, AbcError> {
    let component_type = annotation_array_tag_component_type(tag);
    let mut cursor = value as usize;
    let (count, len) = read_uleb128_at(data, cursor, "annotation array count")?;
    cursor += len;
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(read_array_scalar_value(
            data,
            reader,
            owner_class,
            section,
            class_name_by_offset,
            component_type,
            &mut cursor,
            literal_offset_to_index,
        )?);
    }
    Ok(ArrayValueRecord {
        value_type: 17,
        component_type,
        values,
    })
}

fn read_array_scalar_value(
    data: &[u8],
    reader: &AbcReader<'_>,
    owner_class: &AbcClassItem,
    section: &crate::abc_reader::AbcIndexSection,
    class_name_by_offset: &BTreeMap<u32, String>,
    component_type: u32,
    cursor: &mut usize,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<ScalarValueRecord, AbcError> {
    let value = match component_type {
        VALUE_TYPE_U1 | VALUE_TYPE_I8 | VALUE_TYPE_U8 => {
            let value = *data
                .get(*cursor)
                .ok_or_else(|| abc_data_oob(*cursor, "annotation array u8"))?;
            *cursor += 1;
            ScalarValueData::U64(value as u64)
        }
        VALUE_TYPE_I16 | VALUE_TYPE_U16 => {
            let value = read_at_u16(data, *cursor, "annotation array u16")?;
            *cursor += 2;
            ScalarValueData::U64(value as u64)
        }
        VALUE_TYPE_I32 | VALUE_TYPE_U32 => {
            let value = read_at_u32(data, *cursor, "annotation array u32")?;
            *cursor += 4;
            ScalarValueData::U64(value as u64)
        }
        VALUE_TYPE_F32 => {
            let value = read_at_u32(data, *cursor, "annotation array f32")?;
            *cursor += 4;
            ScalarValueData::F32(f32::from_bits(value))
        }
        VALUE_TYPE_I64 | VALUE_TYPE_U64 => {
            let value = read_at_u64(data, *cursor, "annotation array u64")?;
            *cursor += 8;
            ScalarValueData::U64(value)
        }
        VALUE_TYPE_F64 => {
            let value = read_at_u64(data, *cursor, "annotation array f64")?;
            *cursor += 8;
            ScalarValueData::F64(f64::from_bits(value))
        }
        VALUE_TYPE_STRING => {
            let value = read_at_u32(data, *cursor, "annotation array string")?;
            *cursor += 4;
            ScalarValueData::String(
                reader
                    .string_at(value)
                    .map_err(read_error("<abc annotation array>"))?,
            )
        }
        VALUE_TYPE_ANNOTATION => {
            let value = read_at_u32(data, *cursor, "annotation array annotation")?;
            *cursor += 4;
            let Some((record_name, elements)) = annotation_from_abc(
                data,
                reader,
                owner_class,
                value,
                section,
                class_name_by_offset,
                literal_offset_to_index,
            )?
            else {
                return Ok(ScalarValueRecord {
                    value_type: component_type,
                    value: ScalarValueData::U64(value as u64),
                });
            };
            ScalarValueData::Annotation(Box::new(AnnotationRecord {
                record_name,
                elements,
            }))
        }
        _ => {
            let value = read_at_u32(data, *cursor, "annotation array id")?;
            *cursor += 4;
            ScalarValueData::U64(value as u64)
        }
    };
    Ok(ScalarValueRecord {
        value_type: component_type,
        value,
    })
}

fn read_uleb128_at(data: &[u8], mut cursor: usize, what: &str) -> Result<(u64, usize), AbcError> {
    let start = cursor;
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *data.get(cursor).ok_or_else(|| abc_data_oob(cursor, what))?;
        cursor += 1;
        result |= ((byte & 0x7f) as u64) << shift;
        if (byte & 0x80) == 0 {
            return Ok((result, cursor - start));
        }
        shift += 7;
        if shift >= 64 {
            return Err(abc_data_oob(start, what));
        }
    }
}

fn annotation_tag_value_type(tag: u8) -> u32 {
    match tag {
        b'1' => VALUE_TYPE_U1,
        b'2' => VALUE_TYPE_I8,
        b'3' => VALUE_TYPE_U8,
        b'4' => VALUE_TYPE_I16,
        b'5' => VALUE_TYPE_U16,
        b'6' => VALUE_TYPE_I32,
        b'7' => VALUE_TYPE_U32,
        b'8' => VALUE_TYPE_I64,
        b'9' => VALUE_TYPE_U64,
        b'A' => VALUE_TYPE_F32,
        b'B' => VALUE_TYPE_F64,
        b'C' => VALUE_TYPE_STRING,
        b'D' => VALUE_TYPE_RECORD,
        b'E' => VALUE_TYPE_METHOD,
        b'F' => VALUE_TYPE_ENUM,
        b'G' => VALUE_TYPE_ANNOTATION,
        b'*' => VALUE_TYPE_STRING_NULLPTR,
        b'#' => VALUE_TYPE_LITERALARRAY,
        _ => VALUE_TYPE_U32,
    }
}

fn annotation_array_tag_component_type(tag: u8) -> u32 {
    match tag {
        b'K' | b'H' => VALUE_TYPE_U1,
        b'L' => VALUE_TYPE_I8,
        b'M' => VALUE_TYPE_U8,
        b'N' => VALUE_TYPE_I16,
        b'O' => VALUE_TYPE_U16,
        b'P' => VALUE_TYPE_I32,
        b'Q' => VALUE_TYPE_U32,
        b'R' => VALUE_TYPE_I64,
        b'S' => VALUE_TYPE_U64,
        b'T' => VALUE_TYPE_F32,
        b'U' => VALUE_TYPE_F64,
        b'V' => VALUE_TYPE_STRING,
        b'W' => VALUE_TYPE_RECORD,
        b'X' => VALUE_TYPE_METHOD,
        b'Y' => VALUE_TYPE_ENUM,
        b'Z' => VALUE_TYPE_ANNOTATION,
        b'@' => VALUE_TYPE_METHOD_HANDLE,
        _ => VALUE_TYPE_U32,
    }
}

fn read_at_u16(data: &[u8], offset: usize, what: &str) -> Result<u16, AbcError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| abc_data_oob(offset, what))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_at_u32(data: &[u8], offset: usize, what: &str) -> Result<u32, AbcError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| abc_data_oob(offset, what))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_at_u64(data: &[u8], offset: usize, what: &str) -> Result<u64, AbcError> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or_else(|| abc_data_oob(offset, what))?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn abc_data_oob(offset: usize, what: &str) -> AbcError {
    AbcError::UnsupportedConstruct {
        construct: format!("ABC {what} at {offset:#x} out of bounds"),
        file: "<abc input>".to_string(),
    }
}

#[derive(Debug, Clone, Copy)]
struct AbcMethodCallInfo {
    access_flags: u32,
    proto_num_args: Option<u32>,
    code_num_args: Option<u32>,
}

impl AbcMethodCallInfo {
    fn is_static(self) -> bool {
        const ACC_STATIC: u32 = 0x0008;
        self.access_flags & ACC_STATIC != 0
    }

    fn num_args(self) -> Option<u32> {
        self.proto_num_args.or(self.code_num_args)
    }
}

fn build_method_call_infos(
    reader: &AbcReader<'_>,
    classes: &[AbcClassItem],
) -> Result<BTreeMap<u32, AbcMethodCallInfo>, AbcError> {
    let mut infos = BTreeMap::new();
    for class in classes {
        for method in &class.methods {
            let proto_num_args = if method.proto_idx == u16::MAX {
                None
            } else {
                reader
                    .resolve_proto_index(method.offset, method.proto_idx as u32)
                    .map_err(read_error("<abc input>"))?
                    .map(|proto_off| {
                        reader
                            .proto_num_args(proto_off)
                            .map_err(read_error("<abc input>"))
                    })
                    .transpose()?
            };
            infos.insert(
                method.offset,
                AbcMethodCallInfo {
                    access_flags: method.access_flags,
                    proto_num_args,
                    code_num_args: method.code.as_ref().map(|code| code.num_args),
                },
            );
        }
    }
    Ok(infos)
}

fn field_value_from_abc(
    field_name: &str,
    value: Option<AbcFieldValue>,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<Option<FieldValue>, AbcError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        AbcFieldValue::U8(value) => Ok(Some(FieldValue::U8(value))),
        AbcFieldValue::U32(offset)
            if matches!(
                field_name,
                "moduleRecordIdx" | "scopeNames" | "moduleRequestPhaseIdx"
            ) =>
        {
            let Some(index) = literal_offset_to_index.get(&offset).copied() else {
                return Err(AbcError::UnsupportedConstruct {
                    construct: format!(
                        "missing literal array for field {field_name} at {offset:#x}"
                    ),
                    file: "<abc input>".to_string(),
                });
            };
            Ok(Some(FieldValue::LiteralArray(index)))
        }
        AbcFieldValue::U32(value) => Ok(Some(FieldValue::U32(value))),
    }
}

fn read_tagged_literal_array(
    reader: &AbcReader<'_>,
    offset: u32,
    literal_offset_to_index: &BTreeMap<u32, usize>,
) -> Result<LiteralArray, AbcError> {
    let array = reader
        .read_literal_array_item(offset)
        .map_err(read_error("<abc input>"))?;
    let mut entries = Vec::with_capacity(array.entries.len());
    for entry in array.entries {
        entries.push(match entry {
            AbcLiteralEntry::Bool(value) => LiteralEntry::TaggedBool(value),
            AbcLiteralEntry::U8 {
                tag: LITERAL_TAG_ACCESSOR,
                value,
            } => LiteralEntry::TaggedAccessor(value),
            AbcLiteralEntry::U8 {
                tag: LITERAL_TAG_BUILTIN_TYPE_INDEX,
                value,
            } => LiteralEntry::TaggedBuiltinTypeIndex(value),
            AbcLiteralEntry::U32 {
                tag: LITERAL_TAG_INTEGER,
                value,
            } => LiteralEntry::TaggedInteger(value),
            AbcLiteralEntry::U32 {
                tag: LITERAL_TAG_LITERAL_BUFFER_INDEX,
                value,
            } => LiteralEntry::TaggedLiteralBufferIndex(value),
            AbcLiteralEntry::F32(value) => LiteralEntry::TaggedFloat(value),
            AbcLiteralEntry::F64(value) => LiteralEntry::TaggedDouble(value),
            AbcLiteralEntry::String {
                tag: LITERAL_TAG_STRING,
                value,
                ..
            } => LiteralEntry::TaggedString(value),
            AbcLiteralEntry::String {
                tag: LITERAL_TAG_ETS_IMPLEMENTS,
                value,
                ..
            } => LiteralEntry::TaggedEtsImplements(value),
            AbcLiteralEntry::Method {
                tag: LITERAL_TAG_METHOD,
                offset,
                name,
            } => name
                .map(LiteralEntry::TaggedMethodName)
                .unwrap_or(LiteralEntry::TaggedMethod(offset)),
            AbcLiteralEntry::Method {
                tag: LITERAL_TAG_GENERATOR_METHOD,
                offset,
                name,
            } => name
                .map(LiteralEntry::TaggedGeneratorMethodName)
                .unwrap_or(LiteralEntry::TaggedGeneratorMethod(offset)),
            AbcLiteralEntry::Method {
                tag: LITERAL_TAG_ASYNC_GENERATOR_METHOD,
                offset,
                name,
            } => name
                .map(LiteralEntry::TaggedAsyncGeneratorMethodName)
                .unwrap_or(LiteralEntry::TaggedAsyncGeneratorMethod(offset)),
            AbcLiteralEntry::Method {
                tag: LITERAL_TAG_GETTER,
                offset,
                name,
            } => name
                .map(LiteralEntry::TaggedGetterName)
                .unwrap_or(LiteralEntry::TaggedGetter(offset)),
            AbcLiteralEntry::Method {
                tag: LITERAL_TAG_SETTER,
                offset,
                name,
            } => name
                .map(LiteralEntry::TaggedSetterName)
                .unwrap_or(LiteralEntry::TaggedSetter(offset)),
            AbcLiteralEntry::U16 {
                tag: LITERAL_TAG_METHOD_AFFILIATE,
                value,
            } => LiteralEntry::TaggedMethodAffiliate(value),
            AbcLiteralEntry::LiteralArray { offset } => {
                let Some(index) = literal_offset_to_index.get(&offset).copied() else {
                    return Err(AbcError::UnsupportedConstruct {
                        construct: format!("missing nested literal array at {offset:#x}"),
                        file: format!("{offset:#x}"),
                    });
                };
                LiteralEntry::TaggedLiteralArrayIndex(index)
            }
            AbcLiteralEntry::Null => LiteralEntry::TaggedNull,
            other => {
                return Err(AbcError::UnsupportedConstruct {
                    construct: format!("literal entry {other:?} is not representable yet"),
                    file: format!("{offset:#x}"),
                })
            }
        });
    }
    Ok(LiteralArray {
        entries,
        explicit_key: None,
    })
}

fn read_module_record_literal_array(
    data: &[u8],
    reader: &AbcReader<'_>,
    offset: u32,
) -> Result<LiteralArray, AbcError> {
    let mut cursor = offset as usize;
    let item_count = read_u32(data, &mut cursor)? as usize;
    let mut consumed = 0usize;
    let mut entries = Vec::with_capacity(item_count);

    read_raw_u32(data, &mut cursor, &mut consumed, &mut entries)?;
    let requests = match entries.last() {
        Some(LiteralEntry::RawU32(value)) => *value as usize,
        _ => 0,
    };
    for _ in 0..requests {
        read_raw_string(data, reader, &mut cursor, &mut consumed, &mut entries)?;
    }

    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, reader, cursor, consumed, entries| {
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_u16(data, cursor, consumed, entries)
        },
    )?;
    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, reader, cursor, consumed, entries| {
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_u16(data, cursor, consumed, entries)
        },
    )?;
    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, reader, cursor, consumed, entries| {
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_string(data, reader, cursor, consumed, entries)
        },
    )?;
    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, reader, cursor, consumed, entries| {
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_string(data, reader, cursor, consumed, entries)?;
            read_raw_u16(data, cursor, consumed, entries)
        },
    )?;
    read_counted(
        data,
        reader,
        &mut cursor,
        &mut consumed,
        &mut entries,
        |data, _reader, cursor, consumed, entries| read_raw_u16(data, cursor, consumed, entries),
    )?;

    if consumed != item_count {
        return Err(AbcError::UnsupportedConstruct {
            construct: format!(
                "module literal array item count mismatch: header={item_count}, parsed={consumed}"
            ),
            file: format!("{offset:#x}"),
        });
    }

    Ok(LiteralArray {
        entries,
        explicit_key: None,
    })
}

fn read_counted(
    data: &[u8],
    reader: &AbcReader<'_>,
    cursor: &mut usize,
    consumed: &mut usize,
    entries: &mut Vec<LiteralEntry>,
    mut read_item: impl FnMut(
        &[u8],
        &AbcReader<'_>,
        &mut usize,
        &mut usize,
        &mut Vec<LiteralEntry>,
    ) -> Result<(), AbcError>,
) -> Result<(), AbcError> {
    read_raw_u32(data, cursor, consumed, entries)?;
    let count = match entries.last() {
        Some(LiteralEntry::RawU32(value)) => *value as usize,
        _ => 0,
    };
    for _ in 0..count {
        read_item(data, reader, cursor, consumed, entries)?;
    }
    Ok(())
}

fn read_raw_u32(
    data: &[u8],
    cursor: &mut usize,
    consumed: &mut usize,
    entries: &mut Vec<LiteralEntry>,
) -> Result<(), AbcError> {
    let value = read_u32(data, cursor)?;
    entries.push(LiteralEntry::RawU32(value));
    *consumed += 1;
    Ok(())
}

fn read_raw_u16(
    data: &[u8],
    cursor: &mut usize,
    consumed: &mut usize,
    entries: &mut Vec<LiteralEntry>,
) -> Result<(), AbcError> {
    if cursor.saturating_add(2) > data.len() {
        return Err(AbcError::UnsupportedConstruct {
            construct: "truncated module literal u16".to_string(),
            file: "<abc input>".to_string(),
        });
    }
    let value = u16::from_le_bytes([data[*cursor], data[*cursor + 1]]);
    *cursor += 2;
    entries.push(LiteralEntry::RawU16(value));
    *consumed += 1;
    Ok(())
}

fn read_raw_string(
    data: &[u8],
    reader: &AbcReader<'_>,
    cursor: &mut usize,
    consumed: &mut usize,
    entries: &mut Vec<LiteralEntry>,
) -> Result<(), AbcError> {
    let offset = read_u32(data, cursor)?;
    let value = reader
        .string_at(offset)
        .map_err(read_error("<abc input>"))?;
    entries.push(LiteralEntry::RawString(value));
    *consumed += 1;
    Ok(())
}

fn read_u32(data: &[u8], cursor: &mut usize) -> Result<u32, AbcError> {
    if cursor.saturating_add(4) > data.len() {
        return Err(AbcError::UnsupportedConstruct {
            construct: "truncated ABC u32".to_string(),
            file: "<abc input>".to_string(),
        });
    }
    let value = u32::from_le_bytes([
        data[*cursor],
        data[*cursor + 1],
        data[*cursor + 2],
        data[*cursor + 3],
    ]);
    *cursor += 4;
    Ok(value)
}

fn method_kind_from_access_flags(name: &str, access_flags: u32) -> MethodKind {
    if name == "func_main_0" {
        MethodKind::Module
    } else if name == "constructor" {
        MethodKind::Constructor
    } else if access_flags == method_access::ANON {
        MethodKind::Anonymous
    } else {
        MethodKind::Function
    }
}

fn abc_default_function_kind(kind: MethodKind) -> u32 {
    if kind == MethodKind::Anonymous {
        function_kind::NC_FUNCTION
    } else {
        function_kind::FUNCTION
    }
}

fn unpatch_bytecode_index_operands(
    reader: &AbcReader<'_>,
    file: &mut AbcFile,
    method_off: u32,
    bytecode: &[u8],
    literal_offset_to_index: &BTreeMap<u32, usize>,
    method_names: &BTreeMap<u32, String>,
    record_string_indices: &mut BTreeSet<u32>,
    record_literal_indices: &mut BTreeSet<usize>,
) -> Result<Vec<u8>, AbcError> {
    let pool = reader
        .method_index_for_method(method_off)
        .map_err(read_error("<abc input>"))?
        .unwrap_or_default();
    let mut string_pool = FxHashMap::<u32, u32>::default();
    let mut literal_pool = FxHashMap::<u32, u32>::default();
    let mut method_pool = FxHashMap::<u32, u32>::default();

    for (pool_index, offset) in pool.into_iter().enumerate() {
        let pool_index = pool_index as u32;
        if let Some(&literal_index) = literal_offset_to_index.get(&offset) {
            literal_pool.insert(pool_index, literal_index as u32);
        } else if let Some(name) = method_names.get(&offset) {
            let string_id = file.strings.intern(name);
            record_string_indices.insert(string_id);
            method_pool.insert(pool_index, string_id);
        } else if let Ok(value) = reader.string_at(offset) {
            let string_id = file.strings.intern(value.as_str());
            record_string_indices.insert(string_id);
            string_pool.insert(pool_index, string_id);
        }
    }

    let mut out = bytecode.to_vec();
    let mut pos = 0usize;
    while pos < out.len() {
        let opcode = out[pos];
        match opcode {
            0x3e if pos + 2 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 1, &string_pool);
                pos += 3;
                continue;
            }
            0x3f | 0x40 | 0x42 if pos + 3 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &string_pool);
                pos += 4;
                continue;
            }
            0x43 | 0x7a | 0x8e | 0xdb | 0xdc if pos + 4 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &string_pool);
                pos += 5;
                continue;
            }
            0xfe if out.get(pos + 1) == Some(&0x09) && pos + 3 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &string_pool);
                pos += 4;
                continue;
            }
            0x33 | 0x34 if pos + 4 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &method_pool);
                pos += 5;
                continue;
            }
            0x35 if pos + 8 < out.len() => {
                rewrite_pool_u16(&mut out, pos + 2, &method_pool);
                rewrite_literal_pool_u16(&mut out, pos + 4, &literal_pool, record_literal_indices);
                pos += 9;
                continue;
            }
            0xb6 if pos + 3 < out.len() => {
                rewrite_literal_pool_u16(&mut out, pos + 2, &literal_pool, record_literal_indices);
                pos += 4;
                continue;
            }
            0x06 | 0x07 if pos + 3 < out.len() => {
                rewrite_literal_pool_u16(&mut out, pos + 2, &literal_pool, record_literal_indices);
                pos += 4;
                continue;
            }
            0xfb if out.get(pos + 1) == Some(&0x04) && pos + 5 < out.len() => {
                rewrite_literal_pool_u16(&mut out, pos + 4, &literal_pool, record_literal_indices);
                pos += 6;
                continue;
            }
            _ => {}
        }
        if let Some(len) = rewrite_isa_id_operands(
            &mut out,
            pos,
            &string_pool,
            &method_pool,
            &literal_pool,
            record_literal_indices,
        ) {
            pos += len;
            continue;
        }
        pos += instruction_len(opcode, out.get(pos + 1).copied()).max(1);
    }

    Ok(out)
}

fn rewrite_pool_u16(bytecode: &mut [u8], operand_pos: usize, pool_map: &FxHashMap<u32, u32>) {
    let old = u16::from_le_bytes([bytecode[operand_pos], bytecode[operand_pos + 1]]);
    if let Some(new) = pool_map
        .get(&(old as u32))
        .and_then(|new| u16::try_from(*new).ok())
    {
        bytecode[operand_pos..operand_pos + 2].copy_from_slice(&new.to_le_bytes());
    }
}

fn rewrite_literal_pool_u16(
    bytecode: &mut [u8],
    operand_pos: usize,
    pool_map: &FxHashMap<u32, u32>,
    record_literal_indices: &mut BTreeSet<usize>,
) {
    let old = u16::from_le_bytes([bytecode[operand_pos], bytecode[operand_pos + 1]]);
    if let Some(new) = pool_map
        .get(&(old as u32))
        .and_then(|new| u16::try_from(*new).ok())
    {
        bytecode[operand_pos..operand_pos + 2].copy_from_slice(&new.to_le_bytes());
        record_literal_indices.insert(new as usize);
    }
}

fn rewrite_isa_id_operands(
    bytecode: &mut [u8],
    pos: usize,
    string_pool: &FxHashMap<u32, u32>,
    method_pool: &FxHashMap<u32, u32>,
    literal_pool: &FxHashMap<u32, u32>,
    record_literal_indices: &mut BTreeSet<usize>,
) -> Option<usize> {
    let len = isa_instruction_len(bytecode, pos)?;
    if pos.checked_add(len)? > bytecode.len() {
        return None;
    }
    let operands = isa_id_operands(bytecode, pos)?;
    for operand in operands {
        let operand_pos = pos.checked_add(operand.offset)?;
        let old = read_id_operand(bytecode, operand_pos, operand.width)?;
        let new = match operand.kind {
            IdKind::String => string_pool.get(&old).copied(),
            IdKind::Method => method_pool.get(&old).copied(),
            IdKind::LiteralArray => literal_pool.get(&old).copied().inspect(|idx| {
                record_literal_indices.insert(*idx as usize);
            }),
            IdKind::Other => None,
        };
        if let Some(new) = new {
            write_id_operand(bytecode, operand_pos, operand.width, new)?;
        }
    }
    Some(len)
}

fn read_id_operand(bytecode: &[u8], operand_pos: usize, width: u8) -> Option<u32> {
    match width {
        16 => {
            Some(
                u16::from_le_bytes([*bytecode.get(operand_pos)?, *bytecode.get(operand_pos + 1)?])
                    as u32,
            )
        }
        32 => Some(u32::from_le_bytes([
            *bytecode.get(operand_pos)?,
            *bytecode.get(operand_pos + 1)?,
            *bytecode.get(operand_pos + 2)?,
            *bytecode.get(operand_pos + 3)?,
        ])),
        _ => None,
    }
}

fn write_id_operand(bytecode: &mut [u8], operand_pos: usize, width: u8, value: u32) -> Option<()> {
    match width {
        16 => {
            let value = u16::try_from(value).ok()?;
            bytecode[operand_pos..operand_pos + 2].copy_from_slice(&value.to_le_bytes());
        }
        32 => {
            bytecode[operand_pos..operand_pos + 4].copy_from_slice(&value.to_le_bytes());
        }
        _ => return None,
    }
    Some(())
}

fn decode_bytecode(
    reader: &AbcReader<'_>,
    bytecode: &[u8],
    raw_bytecode: &[u8],
    file: &AbcFile,
    record_name: &str,
    method_off: u32,
    method_call_infos: &BTreeMap<u32, AbcMethodCallInfo>,
) -> Result<Vec<Insn>, AbcError> {
    if raw_bytecode.len() != bytecode.len() {
        return Err(AbcError::UnsupportedConstruct {
            construct: format!(
                "ABC bytecode length changed while materializing Program: raw={}, rewritten={}",
                raw_bytecode.len(),
                bytecode.len()
            ),
            file: "<abc input>".to_string(),
        });
    }
    let mut insns = Vec::new();
    let mut pos = 0usize;
    while pos < bytecode.len() {
        let insn = decode_one(
            reader,
            bytecode,
            raw_bytecode,
            pos,
            file,
            record_name,
            method_off,
            method_call_infos,
        )?;
        let len = insn.byte_len().max(1);
        if pos.saturating_add(len) > bytecode.len() {
            return Err(AbcError::UnsupportedConstruct {
                construct: format!(
                    "truncated ABC bytecode instruction opcode {:#x} at pc {pos}",
                    bytecode[pos]
                ),
                file: "<abc input>".to_string(),
            });
        }
        insns.push(insn);
        pos += len;
    }
    Ok(insns)
}

fn decode_one(
    reader: &AbcReader<'_>,
    bytecode: &[u8],
    raw_bytecode: &[u8],
    pos: usize,
    file: &AbcFile,
    record_name: &str,
    method_off: u32,
    method_call_infos: &BTreeMap<u32, AbcMethodCallInfo>,
) -> Result<Insn, AbcError> {
    let op = bytecode[pos];
    let at = |offset: usize| -> u8 { bytecode[pos + offset] };
    let u16_at = |offset: usize| -> u16 { u16::from_le_bytes([at(offset), at(offset + 1)]) };
    let i16_at = |offset: usize| -> i16 { i16::from_le_bytes([at(offset), at(offset + 1)]) };
    let i32_at = |offset: usize| -> i32 {
        i32::from_le_bytes([at(offset), at(offset + 1), at(offset + 2), at(offset + 3)])
    };
    let u64_at = |offset: usize| -> u64 {
        u64::from_le_bytes([
            at(offset),
            at(offset + 1),
            at(offset + 2),
            at(offset + 3),
            at(offset + 4),
            at(offset + 5),
            at(offset + 6),
            at(offset + 7),
        ])
    };

    let insn = match op {
        0x00 => Insn::LdUndefined,
        0x01 => Insn::LdNull,
        0x02 => Insn::LdTrue,
        0x03 => Insn::LdFalse,
        0x04 => Insn::CreateEmptyObject,
        0x05 => Insn::CreateEmptyArray(at(1) as u16),
        0x06 => Insn::CreateArrayWithBuffer(at(1), u16_at(2)),
        0x07 => Insn::CreateObjectWithBuffer(at(1), u16_at(2)),
        0x08 => Insn::NewObjRange(at(1), at(2), at(3)),
        0x09 => Insn::NewLexEnv(at(1)),
        0x0a => Insn::Add2(at(1), at(2)),
        0x0b => Insn::Sub2(at(1), at(2)),
        0x0c => Insn::Mul2(at(1), at(2)),
        0x0d => Insn::Div2(at(1), at(2)),
        0x0e => Insn::Mod2(at(1), at(2)),
        0x0f => Insn::Eq(at(1), at(2)),
        0x10 => Insn::NotEq(at(1), at(2)),
        0x11 => Insn::Less(at(1), at(2)),
        0x12 => Insn::LessEq(at(1), at(2)),
        0x13 => Insn::Greater(at(1), at(2)),
        0x14 => Insn::GreaterEq(at(1), at(2)),
        0x15 => Insn::Shl2(at(1), at(2)),
        0x16 => Insn::Shr2(at(1), at(2)),
        0x17 => Insn::Ashr2(at(1), at(2)),
        0x18 => Insn::And2(at(1), at(2)),
        0x19 => Insn::Or2(at(1), at(2)),
        0x1a => Insn::Xor2(at(1), at(2)),
        0x1b => Insn::Exp(at(1), at(2)),
        0x1c => Insn::TypeOf(at(1)),
        0x1d => Insn::ToNumber(at(1)),
        0x1e => Insn::ToNumeric(at(1)),
        0x1f => Insn::Neg(at(1)),
        0x20 => Insn::BitNot(at(1)),
        0x21 => Insn::Inc(at(1)),
        0x22 => Insn::Dec(at(1)),
        0x23 => Insn::IsTrue,
        0x24 => Insn::IsFalse,
        0x25 => Insn::IsIn(at(1), at(2)),
        0x26 => Insn::InstanceOf(at(1), at(2)),
        0x27 => Insn::StrictNotEq(at(1), at(2)),
        0x28 => Insn::StrictEq(at(1), at(2)),
        0x29 => Insn::CallArg0(at(1)),
        0x2a => Insn::CallArg1(at(1), at(2)),
        0x2b => Insn::CallArgs2(at(1), at(2), at(3)),
        0x2c => Insn::CallArgs3(at(1), at(2), at(3), at(4)),
        0x2d => Insn::CallThis0(at(1), at(2)),
        0x2e => Insn::CallThis1(at(1), at(2), at(3)),
        0x2f => Insn::CallThis2(at(1), at(2), at(3), at(4)),
        0x30 => Insn::CallThis3(at(1), at(2), at(3), at(4), at(5)),
        0x31 => Insn::CallThisRange(at(1), at(2), at(3)),
        0x32 => Insn::SuperCallThisRange(at(1), at(2), at(3)),
        0x33 => Insn::DefineFuncShort(at(1), u16_at(2), at(4)),
        0x34 => Insn::DefineMethodShort(at(1), u16_at(2), at(4)),
        0x35 => Insn::DefineClassWithBuffer(at(1), u16_at(2), u16_at(4), u16_at(6), at(8)),
        0x36 => Insn::GetNextPropName(at(1)),
        0x37 => Insn::LdObjByValue(at(1), at(2)),
        0x38 => Insn::StObjByValue(at(1), at(2), at(3)),
        0x39 => Insn::LdSuperByValue(at(1), at(2)),
        0x3c => {
            let packed = at(1);
            Insn::LdLexVar(packed & 0x0f, packed >> 4)
        }
        0x3d => {
            let packed = at(1);
            Insn::StLexVar(packed & 0x0f, packed >> 4)
        }
        0x3e => Insn::LdaStr(u16_at(1)),
        0x3f => Insn::TryLdGlobalByName(at(1), u16_at(2)),
        0x40 => Insn::TryStGlobalByName(at(1), u16_at(2)),
        0x42 => Insn::LdObjByName(at(1), u16_at(2)),
        0x43 => Insn::StObjByName(at(1), u16_at(2), at(4)),
        0x44 => {
            let packed = at(1);
            Insn::MovV4(packed & 0x0f, packed >> 4)
        }
        0x45 => Insn::MovV8(at(1), at(2)),
        0x46 => Insn::LdSuperByName(at(1), u16_at(2)),
        0x4d => Insn::JmpShort(at(1) as i8),
        0x4e => Insn::Jmp(i16_at(1)),
        0x4f => Insn::JeqzShort(at(1) as i8),
        0x50 => Insn::Jeqz(i16_at(1)),
        0x51 => Insn::JnezShort(at(1) as i8),
        0x60 => Insn::Lda(at(1)),
        0x61 => Insn::Sta(at(1)),
        0x62 => Insn::LdAi(i32_at(1)),
        0x63 => Insn::FLdAi(f64::from_bits(u64_at(1))),
        0x64 => Insn::Return,
        0x65 => Insn::ReturnUndefined,
        0x66 => Insn::GetPropIterator,
        0x67 => Insn::GetIterator(at(1)),
        0x68 => Insn::CloseIterator(at(1), at(2)),
        0x69 => Insn::PopLexEnv,
        0x6d => Insn::LdGlobal,
        0x6f => Insn::LdThis,
        0x70 => Insn::LdHole,
        0x73 => Insn::CallRange(at(1), at(2), at(3)),
        0x76 => Insn::GetTemplateObject(at(1) as u16),
        0x77 => Insn::SetObjectWithProto(at(1), at(2)),
        0x78 => Insn::StOwnByValue(at(1), at(2), at(3)),
        0x79 => Insn::StOwnByIndex(at(1) as u16, at(2), u16_at(3)),
        0x7a => Insn::StOwnByName(at(1), u16_at(2), at(4)),
        0x7c => Insn::StModuleVar(at(1)),
        0x7d => Insn::LdLocalModuleVar(at(1)),
        0x7e => Insn::LdExternalModuleVar(at(1)),
        0x80 => Insn::CreateEmptyArray(u16_at(1)),
        0x8a => Insn::LdLexVar(at(1), at(2)),
        0x8b => Insn::StLexVar(at(1), at(2)),
        0x8e => Insn::StOwnByNameWithNameSet(at(1), u16_at(2), at(4)),
        0x8f => Insn::MovV16(u16_at(1), u16_at(3)),
        0x98 => Insn::JmpWide(i32_at(1)),
        0x99 => Insn::StOwnByValueWithNameSet(at(1), at(2), at(3)),
        0x9a => Insn::JeqzWide(i32_at(1)),
        0x9b => Insn::Jnez(i16_at(1)),
        0x9c => Insn::JnezWide(i32_at(1)),
        0xa1 | 0xcf => Insn::CopyRestArgs(at(1)),
        0xb0 => Insn::Debugger,
        0xb1 => Insn::CreateGeneratorObj(at(1)),
        0xb3 => Insn::CreateObjectWithExcludedKeys(at(1) as u16, at(2), at(3)),
        0xb4 => Insn::NewObjApply(at(1), at(2)),
        0xb6 => Insn::NewLexEnvWithName(at(1), u16_at(2)),
        0xb9 => Insn::SuperCallSpread(at(1), at(2)),
        0xba => Insn::Apply(at(1), at(2), at(3)),
        0xbb => Insn::SuperCallArrowRange(at(1), at(2), at(3)),
        0xbc => Insn::DefineGetterSetterByValue(at(1), at(2), at(3), at(4)),
        0xbd => Insn::DynamicImport,
        0xbf => Insn::ResumeGenerator,
        0xc0 => Insn::GetResumeMode,
        0xc1 => Insn::GetTemplateObject(u16_at(1)),
        0xc2 => Insn::DelObjProp(at(1)),
        0xc3 => Insn::SuspendGenerator(at(1)),
        0xc4 => Insn::AsyncFunctionAwaitUncaught(at(1)),
        0xc5 => Insn::CopyDataProperties(at(1)),
        0xc6 => Insn::StArraySpread(at(1), at(2)),
        0xc9 => Insn::StSuperByValue(at(1), at(2), at(3)),
        0xcb => Insn::StOwnByIndex(u16_at(1), at(3), u16_at(4)),
        0xcd => Insn::AsyncFunctionResolve(at(1)),
        0xce => Insn::AsyncFunctionReject(at(1)),
        0xd0 => Insn::StSuperByName(at(1), u16_at(2), at(4)),
        0xd3 => Insn::LdBigInt(u16_at(1)),
        0xd5 => Insn::Nop,
        0xd8 => Insn::LdPrivateProperty(at(1), u16_at(2), u16_at(4)),
        0xd9 => Insn::StPrivateProperty(at(1), u16_at(2), u16_at(4), at(6)),
        0xda => Insn::TestIn(at(1), u16_at(2), u16_at(4)),
        0xdb => Insn::DefineFieldByName(at(1), u16_at(2), at(4)),
        0xdc => Insn::DefinePropertyByName(at(1), u16_at(2), at(4)),
        0xae => Insn::AsyncFunctionEnter,
        0xfd if at(1) == 0x00 => Insn::CreateObjectWithExcludedKeys(u16_at(2), at(4), at(5)),
        0xfe => match at(1) {
            0x00 => Insn::Throw,
            0x02 => Insn::ThrowPatternNonCoercible,
            0x05 => Insn::ThrowIfNotObject(at(2)),
            0x07 => Insn::ThrowIfSuperNotCorrectCall(at(2)),
            0x09 => Insn::ThrowUndefinedIfHoleWithName(u16_at(2)),
            sub => {
                return decode_generic_pandasm(
                    bytecode,
                    raw_bytecode,
                    pos,
                    reader,
                    file,
                    record_name,
                    method_off,
                    method_call_infos,
                )?
                .ok_or_else(|| unsupported_prefixed_error("throw", sub, pos));
            }
        },
        0xfb => match at(1) {
            0x01 => Insn::CallRuntimeDefineFieldByValue(at(2), at(3), at(4)),
            0x04 => Insn::CallRuntimeCreatePrivateProperty(u16_at(2), u16_at(4)),
            0x05 => Insn::CallRuntimeDefinePrivateProperty(at(2), u16_at(3), u16_at(5), at(7)),
            0x06 => Insn::CallRuntimeCallInit(at(2), at(3)),
            0x13 => Insn::CallRuntimeIsTrue(at(2)),
            0x14 => Insn::CallRuntimeIsFalse(at(2)),
            sub => {
                return decode_generic_pandasm(
                    bytecode,
                    raw_bytecode,
                    pos,
                    reader,
                    file,
                    record_name,
                    method_off,
                    method_call_infos,
                )?
                .ok_or_else(|| unsupported_prefixed_error("callruntime", sub, pos));
            }
        },
        _ => {
            if let Some(insn) = decode_generic_pandasm(
                bytecode,
                raw_bytecode,
                pos,
                reader,
                file,
                record_name,
                method_off,
                method_call_infos,
            )? {
                return Ok(insn);
            }
            return Err(AbcError::UnsupportedConstruct {
                construct: format!(
                    "unsupported ABC bytecode opcode {op:#x} at pc {pos} for Program materialization"
                ),
                file: "<abc input>".to_string(),
            });
        }
    };
    Ok(insn)
}

fn decode_generic_pandasm(
    bytecode: &[u8],
    raw_bytecode: &[u8],
    pos: usize,
    reader: &AbcReader<'_>,
    file: &AbcFile,
    record_name: &str,
    method_off: u32,
    method_call_infos: &BTreeMap<u32, AbcMethodCallInfo>,
) -> Result<Option<Insn>, AbcError> {
    let Some(mut raw) = decode_pandasm_instruction(bytecode, pos, |kind, index| {
        resolve_pandasm_id(file, record_name, kind, index)
    }) else {
        return Ok(None);
    };
    trim_pandasm_call_regs(
        &mut raw,
        raw_bytecode,
        pos,
        reader,
        method_off,
        method_call_infos,
    )?;
    Ok(Some(Insn::Pandasm(raw)))
}

fn trim_pandasm_call_regs(
    insn: &mut PandasmInsn,
    raw_bytecode: &[u8],
    pos: usize,
    reader: &AbcReader<'_>,
    method_off: u32,
    method_call_infos: &BTreeMap<u32, AbcMethodCallInfo>,
) -> Result<(), AbcError> {
    if !isa_is_call_instruction(raw_bytecode, pos).unwrap_or(false) {
        return Ok(());
    }

    let target_method_off =
        match isa_first_id_operand(raw_bytecode, pos).flatten() {
            Some(id_operand) => {
                let operand_pos = pos.checked_add(id_operand.offset).ok_or_else(|| {
                    AbcError::UnsupportedConstruct {
                        construct: format!("overflowed ABC call id operand at pc {pos}"),
                        file: "<abc input>".to_string(),
                    }
                })?;
                let index = read_id_operand(raw_bytecode, operand_pos, id_operand.width)
                    .ok_or_else(|| AbcError::UnsupportedConstruct {
                        construct: format!("unsupported ABC call id operand width at pc {pos}"),
                        file: "<abc input>".to_string(),
                    })?;
                reader
                    .resolve_method_index(method_off, index)
                    .map_err(read_error("<abc input>"))?
                    .ok_or_else(|| AbcError::UnsupportedConstruct {
                        construct: format!(
                            "missing method index {index} for CALL instruction at pc {pos}"
                        ),
                        file: "<abc input>".to_string(),
                    })?
            }
            None => method_off,
        };

    let info = method_call_infos
        .get(&target_method_off)
        .copied()
        .ok_or_else(|| AbcError::UnsupportedConstruct {
            construct: format!(
                "missing method metadata for CALL target {target_method_off:#x} at pc {pos}"
            ),
            file: "<abc input>".to_string(),
        })?;
    trim_call_regs_for_method(insn, info, pos)
}

fn trim_call_regs_for_method(
    insn: &mut PandasmInsn,
    info: AbcMethodCallInfo,
    pos: usize,
) -> Result<(), AbcError> {
    let num_args = info
        .num_args()
        .ok_or_else(|| AbcError::UnsupportedConstruct {
            construct: format!("missing ProtoItem arg count for CALL instruction at pc {pos}"),
            file: "<abc input>".to_string(),
        })?;
    let overhead = if info.is_static() {
        insn.regs.len() as i64 - num_args as i64
    } else {
        insn.regs.len() as i64 - num_args as i64 - 1
    };
    if overhead < 0 || overhead as usize > insn.regs.len() {
        return Err(AbcError::UnsupportedConstruct {
            construct: format!(
                "invalid CALL register overhead {overhead} at pc {pos}; regs={}, args={num_args}, static={}",
                insn.regs.len(),
                info.is_static(),
            ),
            file: "<abc input>".to_string(),
        });
    }
    let keep = insn.regs.len() - overhead as usize;
    insn.regs.truncate(keep);
    Ok(())
}

fn resolve_pandasm_id(
    file: &AbcFile,
    record_name: &str,
    kind: IdKind,
    index: u32,
) -> Option<Vec<u8>> {
    match kind {
        IdKind::String => file
            .strings
            .get(index)
            .map(|value| value.as_bytes().to_vec()),
        IdKind::Method => file
            .strings
            .get(index)
            .map(|value| full_method_name(record_name, value).into_bytes()),
        IdKind::LiteralArray => Some(literal_array_key(record_name, index as usize).into_bytes()),
        IdKind::Other => None,
    }
}

fn unsupported_prefixed_error(prefix: &str, sub_opcode: u8, pos: usize) -> AbcError {
    AbcError::UnsupportedConstruct {
        construct: format!(
            "unsupported ABC bytecode {prefix} sub-opcode {sub_opcode:#x} at pc {pos} for Program materialization"
        ),
        file: "<abc input>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{trim_call_regs_for_method, AbcMethodCallInfo};
    use crate::ir::opcodes::PandasmInsn;

    fn pandasm_with_regs(regs: &[u32]) -> PandasmInsn {
        PandasmInsn {
            opcode: 0,
            regs: regs.to_vec(),
            ids: Vec::new(),
            imms: Vec::new(),
            bytes: Vec::new(),
        }
    }

    #[test]
    fn trims_static_call_regs_to_proto_arg_count() {
        let mut insn = pandasm_with_regs(&[1, 2, 3, 4]);
        trim_call_regs_for_method(
            &mut insn,
            AbcMethodCallInfo {
                access_flags: 0x0008,
                proto_num_args: Some(2),
                code_num_args: None,
            },
            0,
        )
        .unwrap();
        assert_eq!(insn.regs, vec![1, 2]);
    }

    #[test]
    fn trims_instance_call_regs_with_receiver_slot() {
        let mut insn = pandasm_with_regs(&[1, 2, 3, 4]);
        trim_call_regs_for_method(
            &mut insn,
            AbcMethodCallInfo {
                access_flags: 0,
                proto_num_args: Some(2),
                code_num_args: None,
            },
            0,
        )
        .unwrap();
        assert_eq!(insn.regs, vec![1, 2, 3]);
    }

    #[test]
    fn rejects_negative_call_overhead() {
        let mut insn = pandasm_with_regs(&[1]);
        let error = trim_call_regs_for_method(
            &mut insn,
            AbcMethodCallInfo {
                access_flags: 0x0008,
                proto_num_args: Some(2),
                code_num_args: None,
            },
            7,
        )
        .expect_err("negative overhead must match upstream abc2program failure");
        assert!(error.to_string().contains("invalid CALL register overhead"));
    }
}
