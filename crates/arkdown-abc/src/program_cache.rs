use std::collections::{BTreeMap, BTreeSet};
use std::io;

use crate::emit::file::{
    AbcFile, LiteralArray, LiteralEntry, LITERAL_TAG_ACCESSOR, LITERAL_TAG_ASYNC_GENERATOR_METHOD,
    LITERAL_TAG_BOOL, LITERAL_TAG_BUILTIN_TYPE_INDEX, LITERAL_TAG_DOUBLE,
    LITERAL_TAG_ETS_IMPLEMENTS, LITERAL_TAG_FLOAT, LITERAL_TAG_GENERATOR_METHOD,
    LITERAL_TAG_GETTER, LITERAL_TAG_INTEGER, LITERAL_TAG_LITERAL_ARRAY,
    LITERAL_TAG_LITERAL_BUFFER_INDEX, LITERAL_TAG_METHOD, LITERAL_TAG_METHOD_AFFILIATE,
    LITERAL_TAG_NULL, LITERAL_TAG_SETTER, LITERAL_TAG_STRING, LITERAL_TAG_TAGVALUE,
};
use crate::emit::record::{
    function_kind, method_access, CatchEntry, ClassRecord, DebugInfoRecord, FieldRecord,
    FieldValue, InstructionDebugInfo, MethodKind, MethodLabelStyle, MethodRecord, ProgramLabel,
    FIELD_OWNER_CLASS_IDX,
};
use crate::emit::strings::StringTable;
use crate::ir::debug_lnp::LineTracker;
use crate::ir::opcodes::{Insn, PandasmImm, PandasmInsn};
use crate::isa::{
    encode_pandasm_instruction, pandasm_invalid_opcode, pandasm_opcode_for_bytecode, IdKind,
};
use crate::names::{
    class_name_from_pandasm_record, full_method_name, literal_array_key, literal_array_record_name,
    pandasm_record_name, short_method_name,
};
use crate::proto_panda::{
    annotation_element, ins, program, scalar_value, variant_value, AnnotationData,
    AnnotationElement, DebuginfoIns, Field, FieldMetadata, Function, FunctionMetadata, Ins,
    ItemMetadata, Literal, Metadata, Parameter, Program, Record, RecordMetadata, ScalarValue,
    SourceLocation, SourcePosition, Type, Value, VariantValue,
};

const VALUE_TYPE_U8: u32 = 2;
const VALUE_TYPE_U32: u32 = 6;
const VALUE_TYPE_LITERALARRAY: u32 = 20;

const VARIANT_BOOL: i32 = variant_value::VariantValueType::Bool as i32;
const VARIANT_U8: i32 = variant_value::VariantValueType::U8 as i32;
const VARIANT_U16: i32 = variant_value::VariantValueType::U16 as i32;
const VARIANT_U32: i32 = variant_value::VariantValueType::U32 as i32;
const VARIANT_F32: i32 = variant_value::VariantValueType::F32 as i32;
const VARIANT_F64: i32 = variant_value::VariantValueType::F64 as i32;
const VARIANT_STRING: i32 = variant_value::VariantValueType::String as i32;

const SCALAR_UINT64: i32 = scalar_value::VariantValueType::Uint64 as i32;
const SCALAR_STRING: i32 = scalar_value::VariantValueType::String as i32;

const CONCURRENT_MODULE_REQUEST_RECORD_NAME: &str = "_ESConcurrentModuleRequestsAnnotation";
const EXPECTED_PROPERTY_COUNT_RECORD_NAME: &str = "_ESExpectedPropertyCountAnnotation";
const SLOT_NUMBER_RECORD_NAME: &str = "_ESSlotNumberAnnotation";
const ANNOTATION_RECORD_ACCESS_FLAGS: u32 = 0x2001;

pub(crate) fn program_for_record(
    file: &AbcFile,
    record_index: usize,
    target_api_version: u32,
) -> io::Result<Option<Program>> {
    let Some(record) = file.records.get(record_index) else {
        return Ok(None);
    };
    if record
        .methods
        .iter()
        .any(|method| method.pandasm_insns.is_empty() && !method.bytecode.is_empty())
    {
        return Ok(None);
    }

    let lang = record.source_lang as u32;
    let mut literal_indices = BTreeSet::new();
    for &idx in &record.module_literal_indices {
        literal_indices.insert(idx);
    }
    for &idx in &record.literal_indices {
        literal_indices.insert(idx);
    }

    let mut program = Program {
        lang,
        record_table: Vec::new(),
        function_table: Vec::new(),
        function_synonyms: Vec::new(),
        literal_array_table: Vec::new(),
        strings: record_program_strings(record, file),
        array_types: Vec::new(),
    };

    let mut record_table = BTreeMap::new();
    push_annotation_record_tables(
        &mut record_table,
        include_expected_record(record, target_api_version),
    );
    insert_record_table(
        &mut record_table,
        bytes(&pandasm_record_name(&record.name)),
        record_to_proto(record, file),
    );
    program.record_table = record_table
        .into_iter()
        .map(|(key, value)| program::RecordTable {
            key,
            value: Some(value),
        })
        .collect();

    let mut function_table = BTreeMap::new();
    for method in &record.methods {
        let function = function_to_proto(method, record, &file.strings)?;
        insert_function_table(
            &mut function_table,
            bytes(&full_method_name(&record.name, &method.name)),
            function,
        );
    }
    program.function_table = function_table
        .into_iter()
        .map(|(key, value)| program::FunctionTable {
            key,
            value: Some(value),
        })
        .collect();

    let mut literal_array_table = BTreeMap::new();
    for idx in literal_indices {
        let Some(array) = file.literal_arrays.get(idx) else {
            continue;
        };
        insert_literal_array_table(
            &mut literal_array_table,
            bytes(&literal_key(record, idx)),
            literal_array_to_proto(array, &record.name, is_raw_module_literal_array(array))?,
        );
    }
    program.literal_array_table = literal_array_table
        .into_iter()
        .map(|(key, value)| program::LiteralArrayTable {
            key,
            value: Some(value),
        })
        .collect();

    Ok(Some(program))
}

pub(crate) fn program_for_file(file: &AbcFile, target_api_version: u32) -> io::Result<Program> {
    if file
        .records
        .iter()
        .flat_map(|record| &record.methods)
        .any(|method| method.pandasm_insns.is_empty() && !method.bytecode.is_empty())
    {
        return invalid_data(
            "cannot materialize upstream Program proto for records without pandasm instructions",
        );
    }

    let lang = file
        .records
        .first()
        .map(|record| record.source_lang as u32)
        .unwrap_or_default();
    let mut literal_indices = BTreeSet::new();
    let mut literal_keys = BTreeMap::new();
    let mut literal_record_names = BTreeMap::new();
    for record in &file.records {
        for &idx in &record.module_literal_indices {
            literal_indices.insert(idx);
            literal_keys
                .entry(idx)
                .or_insert_with(|| literal_key(record, idx));
            literal_record_names
                .entry(idx)
                .or_insert_with(|| record.name.clone());
        }
        for &idx in &record.literal_indices {
            literal_indices.insert(idx);
            literal_keys
                .entry(idx)
                .or_insert_with(|| literal_key(record, idx));
            literal_record_names
                .entry(idx)
                .or_insert_with(|| record.name.clone());
        }
    }

    let mut program = Program {
        lang,
        record_table: Vec::new(),
        function_table: Vec::new(),
        function_synonyms: Vec::new(),
        literal_array_table: Vec::new(),
        strings: sorted_program_strings(file.strings.iter()),
        array_types: Vec::new(),
    };

    let mut record_table = BTreeMap::new();
    let include_expected = file
        .records
        .iter()
        .any(|record| include_expected_record(record, target_api_version));
    push_annotation_record_tables(&mut record_table, include_expected);
    let mut function_table = BTreeMap::new();
    for record in &file.records {
        insert_record_table(
            &mut record_table,
            bytes(&pandasm_record_name(&record.name)),
            record_to_proto(record, file),
        );
        for method in &record.methods {
            let function = function_to_proto(method, record, &file.strings)?;
            insert_function_table(
                &mut function_table,
                bytes(&full_method_name(&record.name, &method.name)),
                function,
            );
        }
    }
    program.record_table = record_table
        .into_iter()
        .map(|(key, value)| program::RecordTable {
            key,
            value: Some(value),
        })
        .collect();
    program.function_table = function_table
        .into_iter()
        .map(|(key, value)| program::FunctionTable {
            key,
            value: Some(value),
        })
        .collect();

    let mut literal_array_table = BTreeMap::new();
    for idx in literal_indices {
        let Some(array) = file.literal_arrays.get(idx) else {
            continue;
        };
        insert_literal_array_table(
            &mut literal_array_table,
            bytes(
                literal_keys
                    .get(&idx)
                    .map(String::as_str)
                    .unwrap_or_default(),
            ),
            literal_array_to_proto(
                array,
                literal_record_names
                    .get(&idx)
                    .map(String::as_str)
                    .unwrap_or_default(),
                is_raw_module_literal_array(array),
            )?,
        );
    }
    program.literal_array_table = literal_array_table
        .into_iter()
        .map(|(key, value)| program::LiteralArrayTable {
            key,
            value: Some(value),
        })
        .collect();

    Ok(program)
}

pub(crate) fn append_program_to_file(
    file: &mut AbcFile,
    program: Program,
) -> io::Result<Vec<usize>> {
    for value in &program.strings {
        file.strings.intern(utf8(value)?.as_str());
    }

    let module_literal_keys = collect_module_literal_keys(&program)?;
    let mut literal_key_to_index = BTreeMap::new();
    let mut literal_method_refs = BTreeSet::new();
    let literal_base_index = file.literal_arrays.len();
    let literal_entries = program.literal_array_table;
    for (index, entry) in literal_entries.iter().enumerate() {
        literal_key_to_index.insert(utf8(&entry.key)?, literal_base_index + index);
    }
    for entry in literal_entries {
        let key = utf8(&entry.key)?;
        let Some(array) = entry.value else {
            continue;
        };
        let is_module_literal = module_literal_keys.contains(&key);
        let literal = literal_array_from_proto(
            array,
            is_module_literal,
            literal_array_record_name(&key),
            &literal_key_to_index,
            &mut literal_method_refs,
        )?;
        file.literal_arrays.push(literal);
    }

    let functions = program
        .function_table
        .into_iter()
        .filter_map(|entry| entry.value.map(|function| (entry.key, function)))
        .collect::<Vec<_>>();
    let mut appended = Vec::new();

    for record_entry in program.record_table {
        let Some(record_proto) = record_entry.value else {
            continue;
        };
        let proto_record_name = if record_proto.name.is_empty() {
            utf8(&record_entry.key)?
        } else {
            utf8(&record_proto.name)?
        };
        if is_pandasm_annotation_stub(&proto_record_name) {
            continue;
        }
        let record_name = class_name_from_pandasm_record(&proto_record_name);
        let record_index = file.records.len();
        let mut function_refs = FunctionRefs::default();
        let mut literal_refs = BTreeSet::new();
        let mut methods = Vec::with_capacity(functions.len());

        for (function_key, function) in &functions {
            if !function_belongs_to_record(function_key, function, &record_name)? {
                continue;
            }
            let method = method_from_proto(
                function_key,
                function,
                &record_name,
                &mut file.strings,
                &literal_key_to_index,
                &mut function_refs,
                &mut literal_refs,
            )?;
            methods.push(method);
        }

        function_refs
            .method_pool_entries
            .extend(literal_method_refs.iter().cloned());
        for method in &mut methods {
            finalize_method_kind_and_pool(method, &function_refs);
        }

        let mut module_literal_indices = BTreeSet::new();
        let fields = record_proto
            .field_list
            .iter()
            .map(|field| {
                field_from_proto(field, &literal_key_to_index, &mut module_literal_indices)
            })
            .collect::<io::Result<Vec<_>>>()?;
        literal_refs.retain(|idx| !module_literal_indices.contains(idx));

        file.records.push(ClassRecord {
            name: record_name,
            methods,
            string_indices: Vec::new(),
            literal_indices: literal_refs.into_iter().collect(),
            module_literal_indices: module_literal_indices.into_iter().collect(),
            source_code_strings: functions
                .iter()
                .filter(|(_, function)| !function.source_code.is_empty())
                .map(|(_, function)| utf8(&function.source_code))
                .collect::<io::Result<Vec<_>>>()?,
            access_flags: record_access_flags(&record_proto),
            source_lang: record_proto.language as u8,
            standalone_ecmascript_input: false,
            source_file: (!record_proto.source_file.is_empty())
                .then(|| utf8(&record_proto.source_file))
                .transpose()?,
            module_requests: Vec::new(),
            fields,
        });
        appended.push(record_index);
    }

    Ok(appended)
}

fn collect_module_literal_keys(program: &Program) -> io::Result<BTreeSet<String>> {
    let mut keys = BTreeSet::new();
    for record in &program.record_table {
        let Some(record) = &record.value else {
            continue;
        };
        for field in &record.field_list {
            let field_name = utf8(&field.name)?;
            if !matches!(
                field_name.as_str(),
                "moduleRecordIdx" | "moduleRequestPhaseIdx"
            ) {
                continue;
            }
            let Some(value) = field
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.value.as_ref())
            else {
                continue;
            };
            if value.father.as_ref().map(|father| father.r#type) == Some(VALUE_TYPE_LITERALARRAY) {
                if let Some(scalar_value::Value::ValueStr(bytes)) = &value.value {
                    keys.insert(utf8(bytes)?);
                }
            }
        }
    }
    Ok(keys)
}

fn include_expected_record(record: &ClassRecord, target_api_version: u32) -> bool {
    target_api_version >= 18
        || record
            .methods
            .iter()
            .any(|method| method.expected_property_count.is_some())
}

fn record_program_strings(record: &ClassRecord, file: &AbcFile) -> Vec<Vec<u8>> {
    sorted_program_strings(
        record
            .string_indices
            .iter()
            .filter_map(|idx| file.strings.get(*idx)),
    )
}

fn sorted_program_strings<'a>(strings: impl IntoIterator<Item = &'a str>) -> Vec<Vec<u8>> {
    strings
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(bytes)
        .collect()
}

fn is_raw_module_literal_array(array: &LiteralArray) -> bool {
    !array.entries.is_empty()
        && array.entries.iter().all(|entry| {
            matches!(
                entry,
                LiteralEntry::RawU32(_) | LiteralEntry::RawU16(_) | LiteralEntry::RawString(_)
            )
        })
}

fn insert_record_table(table: &mut BTreeMap<Vec<u8>, Record>, key: Vec<u8>, value: Record) {
    table.entry(key).or_insert(value);
}

fn insert_function_table(table: &mut BTreeMap<Vec<u8>, Function>, key: Vec<u8>, value: Function) {
    table.entry(key).or_insert(value);
}

fn insert_literal_array_table(
    table: &mut BTreeMap<Vec<u8>, crate::proto_panda::LiteralArray>,
    key: Vec<u8>,
    value: crate::proto_panda::LiteralArray,
) {
    table.entry(key).or_insert(value);
}

fn push_annotation_record_tables(
    record_table: &mut BTreeMap<Vec<u8>, Record>,
    has_expected_property_count: bool,
) {
    for name in annotation_record_names(has_expected_property_count) {
        insert_record_table(record_table, bytes(name), annotation_record_to_proto(name));
    }
}

fn annotation_record_names(has_expected_property_count: bool) -> Vec<&'static str> {
    let mut names = Vec::with_capacity(if has_expected_property_count { 3 } else { 2 });
    names.push(CONCURRENT_MODULE_REQUEST_RECORD_NAME);
    if has_expected_property_count {
        names.push(EXPECTED_PROPERTY_COUNT_RECORD_NAME);
    }
    names.push(SLOT_NUMBER_RECORD_NAME);
    names
}

fn annotation_record_to_proto(name: &str) -> Record {
    Record {
        name: bytes(name),
        conflict: false,
        language: 0,
        metadata: Some(record_metadata(ANNOTATION_RECORD_ACCESS_FLAGS)),
        field_list: Vec::new(),
        params_num: 0,
        body_presence: false,
        body_location: None,
        source_file: Vec::new(),
        file_location: None,
    }
}

fn is_pandasm_annotation_stub(name: &str) -> bool {
    matches!(
        name,
        CONCURRENT_MODULE_REQUEST_RECORD_NAME
            | EXPECTED_PROPERTY_COUNT_RECORD_NAME
            | SLOT_NUMBER_RECORD_NAME
    )
}

fn function_belongs_to_record(
    function_key: &[u8],
    function: &Function,
    record_name: &str,
) -> io::Result<bool> {
    let name = if function.name.is_empty() {
        utf8(function_key)?
    } else {
        utf8(&function.name)?
    };
    Ok(short_method_name(record_name, &name) != name)
}

fn literal_array_from_proto(
    array: crate::proto_panda::LiteralArray,
    module_style: bool,
    record_name: Option<&str>,
    literal_key_to_index: &BTreeMap<String, usize>,
    method_refs: &mut BTreeSet<String>,
) -> io::Result<LiteralArray> {
    if module_style {
        let mut entries = Vec::with_capacity(array.literals.len());
        for literal in array.literals {
            entries.push(module_literal_from_proto(literal)?);
        }
        return Ok(LiteralArray { entries });
    }

    if array.literals.len() % 2 != 0 {
        return invalid_data("tagged literal array must contain TAGVALUE/value pairs");
    }
    let mut entries = Vec::with_capacity(array.literals.len() / 2);
    let mut literals = array.literals.into_iter();
    while let Some(tag_literal) = literals.next() {
        let tag = tag_value_from_proto(tag_literal)?;
        let value_literal = literals.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing tagged literal value")
        })?;
        entries.push(tagged_literal_from_proto(
            tag,
            value_literal,
            record_name,
            literal_key_to_index,
            method_refs,
        )?);
    }
    Ok(LiteralArray { entries })
}

fn module_literal_from_proto(literal: Literal) -> io::Result<LiteralEntry> {
    let Some(value) = literal.value else {
        return invalid_data("missing literal value");
    };
    match (literal.tag as u8, value.value, value.r#type) {
        (LITERAL_TAG_INTEGER, Some(variant_value::Value::ValueInt(value)), _) => {
            Ok(LiteralEntry::RawU32(to_u32(value, "module literal u32")?))
        }
        (LITERAL_TAG_METHOD_AFFILIATE, Some(variant_value::Value::ValueInt(value)), _) => {
            Ok(LiteralEntry::RawU16(to_u16(value, "module literal u16")?))
        }
        (LITERAL_TAG_STRING, Some(variant_value::Value::ValueStr(value)), _) => {
            Ok(LiteralEntry::RawString(utf8(&value)?))
        }
        (tag, _, _) => invalid_data(format!("unsupported module literal tag {tag:#x}")),
    }
}

fn tag_value_from_proto(literal: Literal) -> io::Result<u8> {
    let Some(value) = literal.value else {
        return invalid_data("missing literal TAGVALUE value");
    };
    match (literal.tag as u8, value.value, value.r#type) {
        (LITERAL_TAG_TAGVALUE, Some(variant_value::Value::ValueInt(value)), VARIANT_U8) => {
            to_u8(value, "literal TAGVALUE")
        }
        (tag, _, _) => invalid_data(format!("expected TAGVALUE literal, got tag {tag:#x}")),
    }
}

fn tagged_literal_from_proto(
    tag: u8,
    literal: Literal,
    record_name: Option<&str>,
    literal_key_to_index: &BTreeMap<String, usize>,
    method_refs: &mut BTreeSet<String>,
) -> io::Result<LiteralEntry> {
    let Some(value) = literal.value else {
        return invalid_data("missing literal value");
    };
    if literal.tag as u8 != tag {
        return invalid_data(format!(
            "literal TAGVALUE {tag:#x} does not match value tag {:#x}",
            literal.tag
        ));
    }
    match (tag, value.value, value.r#type) {
        (LITERAL_TAG_BOOL, Some(variant_value::Value::ValueInt(value)), _) => {
            Ok(LiteralEntry::TaggedBool(value != 0))
        }
        (LITERAL_TAG_INTEGER, Some(variant_value::Value::ValueInt(value)), _) => Ok(
            LiteralEntry::TaggedInteger(to_u32(value, "literal integer")?),
        ),
        (LITERAL_TAG_FLOAT, Some(variant_value::Value::ValueFloat(value)), _) => {
            Ok(LiteralEntry::TaggedFloat(value))
        }
        (LITERAL_TAG_DOUBLE, Some(variant_value::Value::ValueDouble(value)), _) => {
            Ok(LiteralEntry::TaggedDouble(value))
        }
        (LITERAL_TAG_STRING, Some(variant_value::Value::ValueStr(value)), _) => {
            Ok(LiteralEntry::TaggedString(utf8(&value)?))
        }
        (LITERAL_TAG_METHOD, Some(variant_value::Value::ValueInt(value)), _) => {
            Ok(LiteralEntry::TaggedMethod(to_u32(value, "literal method")?))
        }
        (LITERAL_TAG_METHOD, Some(variant_value::Value::ValueStr(value)), _) => {
            let name = method_literal_name(record_name, &value)?;
            method_refs.insert(name.clone());
            Ok(LiteralEntry::TaggedMethodName(name))
        }
        (LITERAL_TAG_GENERATOR_METHOD, Some(variant_value::Value::ValueInt(value)), _) => Ok(
            LiteralEntry::TaggedGeneratorMethod(to_u32(value, "literal generator method")?),
        ),
        (LITERAL_TAG_GENERATOR_METHOD, Some(variant_value::Value::ValueStr(value)), _) => {
            let name = method_literal_name(record_name, &value)?;
            method_refs.insert(name.clone());
            Ok(LiteralEntry::TaggedGeneratorMethodName(name))
        }
        (LITERAL_TAG_ASYNC_GENERATOR_METHOD, Some(variant_value::Value::ValueInt(value)), _) => {
            Ok(LiteralEntry::TaggedAsyncGeneratorMethod(to_u32(
                value,
                "literal async generator method",
            )?))
        }
        (LITERAL_TAG_ASYNC_GENERATOR_METHOD, Some(variant_value::Value::ValueStr(value)), _) => {
            let name = method_literal_name(record_name, &value)?;
            method_refs.insert(name.clone());
            Ok(LiteralEntry::TaggedAsyncGeneratorMethodName(name))
        }
        (LITERAL_TAG_GETTER, Some(variant_value::Value::ValueInt(value)), _) => {
            Ok(LiteralEntry::TaggedGetter(to_u32(value, "literal getter")?))
        }
        (LITERAL_TAG_GETTER, Some(variant_value::Value::ValueStr(value)), _) => {
            let name = method_literal_name(record_name, &value)?;
            method_refs.insert(name.clone());
            Ok(LiteralEntry::TaggedGetterName(name))
        }
        (LITERAL_TAG_SETTER, Some(variant_value::Value::ValueInt(value)), _) => {
            Ok(LiteralEntry::TaggedSetter(to_u32(value, "literal setter")?))
        }
        (LITERAL_TAG_SETTER, Some(variant_value::Value::ValueStr(value)), _) => {
            let name = method_literal_name(record_name, &value)?;
            method_refs.insert(name.clone());
            Ok(LiteralEntry::TaggedSetterName(name))
        }
        (LITERAL_TAG_ACCESSOR, Some(variant_value::Value::ValueInt(value)), _) => Ok(
            LiteralEntry::TaggedAccessor(to_u8(value, "literal accessor")?),
        ),
        (LITERAL_TAG_METHOD_AFFILIATE, Some(variant_value::Value::ValueInt(value)), _) => Ok(
            LiteralEntry::TaggedMethodAffiliate(to_u16(value, "literal method affiliate")?),
        ),
        (LITERAL_TAG_LITERAL_BUFFER_INDEX, Some(variant_value::Value::ValueInt(value)), _) => Ok(
            LiteralEntry::TaggedLiteralBufferIndex(to_u32(value, "literal buffer index")?),
        ),
        (LITERAL_TAG_LITERAL_ARRAY, Some(variant_value::Value::ValueStr(value)), _) => {
            let key = utf8(&value)?;
            let Some(&index) = literal_key_to_index.get(&key) else {
                return invalid_data(format!("missing nested literal array key {key}"));
            };
            Ok(LiteralEntry::TaggedLiteralArrayIndex(index))
        }
        (LITERAL_TAG_BUILTIN_TYPE_INDEX, Some(variant_value::Value::ValueInt(value)), _) => Ok(
            LiteralEntry::TaggedBuiltinTypeIndex(to_u8(value, "literal builtin type index")?),
        ),
        (LITERAL_TAG_ETS_IMPLEMENTS, Some(variant_value::Value::ValueStr(value)), _) => {
            Ok(LiteralEntry::TaggedEtsImplements(utf8(&value)?))
        }
        (LITERAL_TAG_NULL, _, _) => Ok(LiteralEntry::TaggedNull),
        (tag, _, _) => invalid_data(format!("unsupported literal tag {tag:#x}")),
    }
}

fn method_literal_name(record_name: Option<&str>, value: &[u8]) -> io::Result<String> {
    let value = utf8(value)?;
    Ok(record_name
        .map(|record_name| short_method_name(record_name, &value))
        .unwrap_or(value))
}

fn field_from_proto(
    field: &Field,
    literal_key_to_index: &BTreeMap<String, usize>,
    module_literal_indices: &mut BTreeSet<usize>,
) -> io::Result<FieldRecord> {
    let field_type = field
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.field_type.as_ref())
        .or(field.r#type.as_ref());
    let value = field
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.value.as_ref())
        .map(|value| field_value_from_proto(value, literal_key_to_index, module_literal_indices))
        .transpose()?;
    Ok(FieldRecord {
        name: utf8(&field.name)?,
        class_idx: FIELD_OWNER_CLASS_IDX,
        type_idx: field_type.map(type_idx).unwrap_or(0),
        access_flags: field_access_flags(field),
        value,
    })
}

fn field_value_from_proto(
    value: &ScalarValue,
    literal_key_to_index: &BTreeMap<String, usize>,
    module_literal_indices: &mut BTreeSet<usize>,
) -> io::Result<FieldValue> {
    match value.father.as_ref().map(|father| father.r#type) {
        Some(VALUE_TYPE_U8) => Ok(FieldValue::U8(to_u8(
            scalar_u64_value(value)?,
            "field u8 value",
        )?)),
        Some(VALUE_TYPE_U32) => Ok(FieldValue::U32(to_u32(
            scalar_u64_value(value)?,
            "field u32 value",
        )?)),
        Some(VALUE_TYPE_LITERALARRAY) => {
            let key = scalar_string_value(value)?;
            let Some(&index) = literal_key_to_index.get(&key) else {
                return invalid_data(format!("missing literal array key {key}"));
            };
            module_literal_indices.insert(index);
            Ok(FieldValue::LiteralArray(index))
        }
        other => invalid_data(format!("unsupported field value type {other:?}")),
    }
}

#[derive(Default)]
struct FunctionRefs {
    constructor_names: BTreeSet<String>,
    method_pool_entries: BTreeSet<String>,
    definefunc_order: BTreeMap<String, u32>,
    next_definefunc_order: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JumpKind {
    Jmp,
    Jeqz,
    Jnez,
}

enum DecodedItem {
    Label(String),
    Insn {
        insn: Insn,
        jump: Option<(JumpKind, String)>,
    },
}

fn method_from_proto(
    function_key: &[u8],
    function: &Function,
    record_name: &str,
    strings: &mut StringTable,
    literal_key_to_index: &BTreeMap<String, usize>,
    refs: &mut FunctionRefs,
    literal_refs: &mut BTreeSet<usize>,
) -> io::Result<MethodRecord> {
    let name = if function.name.is_empty() {
        utf8(function_key)?
    } else {
        utf8(&function.name)?
    };
    let name = short_method_name(record_name, &name);
    let (pandasm_insns, pandasm_debug, line_ops, catch_entries, program_labels, method_refs) =
        function_insns_from_proto(function, record_name, strings, literal_key_to_index)?;
    refs.constructor_names
        .extend(method_refs.constructor_names.iter().cloned());
    refs.method_pool_entries
        .extend(method_refs.method_pool_entries.iter().cloned());
    for definefunc in &method_refs.definefunc_order {
        refs.method_pool_entries.insert(definefunc.clone());
        refs.definefunc_order
            .entry(definefunc.clone())
            .or_insert_with(|| {
                let order = refs.next_definefunc_order;
                refs.next_definefunc_order = refs.next_definefunc_order.saturating_add(1);
                order
            });
    }
    literal_refs.extend(method_refs.literal_indices);

    let mut string_table = StringTable::default();
    let bytecode = pandasm_insns
        .iter()
        .flat_map(|insn| insn.encode(&mut string_table))
        .collect();
    let kind = method_kind_from_proto(
        &name,
        function_access_flags(function),
        function.function_kind,
    );

    Ok(MethodRecord {
        name,
        bytecode,
        pandasm_insns,
        pandasm_debug,
        num_vregs: to_u16(function.regs_num, "function regs_num")?,
        num_args: to_u16(function.params.len() as u64, "function params")?,
        debug_info: if function.source_file.is_empty() {
            None
        } else {
            Some(DebugInfoRecord {
                line_start: 1,
                source_file: utf8(&function.source_file)?,
                param_names: Vec::new(),
                lexical_this: false,
            })
        },
        line_ops,
        catch_entries,
        program_labels,
        kind,
        function_kind: function.function_kind,
        concurrent_module_requests: function.concurrent_module_requests.clone(),
        method_pool_entry: false,
        expected_property_count: function_expected_property_count(function)?,
        definefunc_order: None,
        label_style: MethodLabelStyle::Source,
    })
}

fn finalize_method_kind_and_pool(method: &mut MethodRecord, refs: &FunctionRefs) {
    if refs.constructor_names.contains(&method.name) {
        method.kind = MethodKind::Constructor;
    } else if method.name == "func_main_0" {
        method.kind = MethodKind::Module;
    }
    method.method_pool_entry = refs.method_pool_entries.contains(&method.name)
        || matches!(
            method.kind,
            MethodKind::FunctionLiteral | MethodKind::Anonymous | MethodKind::Constructor
        );
    if matches!(
        method.kind,
        MethodKind::FunctionLiteral | MethodKind::Anonymous
    ) {
        method.definefunc_order = refs.definefunc_order.get(&method.name).copied();
    }
}

fn function_insns_from_proto(
    function: &Function,
    record_name: &str,
    strings: &mut StringTable,
    literal_key_to_index: &BTreeMap<String, usize>,
) -> io::Result<(
    Vec<Insn>,
    Vec<InstructionDebugInfo>,
    Vec<u8>,
    Vec<CatchEntry>,
    Vec<ProgramLabel>,
    MethodDecodeRefs,
)> {
    let mut items = Vec::with_capacity(function.ins.len());
    let mut pandasm_debug = Vec::new();
    let mut refs = MethodDecodeRefs::default();
    for ins in &function.ins {
        if ins.set_label_val {
            items.push(DecodedItem::Label(utf8(&ins.label)?));
            continue;
        }
        let decoded = insn_from_proto(ins, record_name, strings, literal_key_to_index, &mut refs)?;
        items.push(decoded);
        if let Some(debug) = instruction_debug_from_proto(ins)? {
            pandasm_debug.push(debug);
        }
    }

    let (pandasm_insns, label_offsets, program_labels) = resolve_jump_labels(&mut items)?;
    if pandasm_debug.len() != pandasm_insns.len() {
        pandasm_debug.clear();
    }
    let line_ops = if !function.source_file.is_empty() && !pandasm_debug.is_empty() {
        line_ops_from_instruction_debug(&pandasm_insns, &pandasm_debug)
    } else {
        Vec::new()
    };
    let catch_entries = function
        .catch_blocks
        .iter()
        .map(|catch| catch_entry_from_proto(catch, &label_offsets))
        .collect::<io::Result<Vec<_>>>()?;
    Ok((
        pandasm_insns,
        pandasm_debug,
        line_ops,
        catch_entries,
        program_labels,
        refs,
    ))
}

fn instruction_debug_from_proto(ins: &Ins) -> io::Result<Option<InstructionDebugInfo>> {
    let Some(debug) = &ins.ins_debug else {
        return Ok(None);
    };
    Ok(Some(InstructionDebugInfo {
        line_number: instruction_debug_line(debug.line_number)?,
        column_number: debug.column_number,
    }))
}

fn instruction_debug_line(line_number: u64) -> io::Result<u32> {
    if line_number == u64::MAX {
        return Ok(u32::MAX);
    }
    to_u32(line_number, "instruction debug line number")
}

fn line_ops_from_instruction_debug(insns: &[Insn], debug: &[InstructionDebugInfo]) -> Vec<u8> {
    if debug.is_empty() {
        return Vec::new();
    }
    let mut tracker = LineTracker::new(u32::MAX);
    for (insn, debug) in insns.iter().zip(debug) {
        tracker.advance_with_col(
            debug.line_number,
            debug.column_number,
            insn.byte_len() as u32,
        );
    }
    tracker.finish()
}

#[derive(Default)]
struct MethodDecodeRefs {
    constructor_names: BTreeSet<String>,
    method_pool_entries: BTreeSet<String>,
    definefunc_order: Vec<String>,
    literal_indices: BTreeSet<usize>,
}

fn resolve_jump_labels(
    items: &mut [DecodedItem],
) -> io::Result<(Vec<Insn>, BTreeMap<String, usize>, Vec<ProgramLabel>)> {
    for _ in 0..8 {
        let (offsets, labels) = item_offsets(items);
        let mut changed = false;
        let mut insn_index = 0usize;
        for item in items.iter_mut() {
            let DecodedItem::Insn { insn, jump } = item else {
                continue;
            };
            if let Some((kind, label)) = jump {
                let Some(&target) = labels.get(label) else {
                    return invalid_data(format!("missing jump label {label}"));
                };
                let offset = offsets[insn_index];
                let rel = target as i64 - offset as i64;
                let new_insn = jump_insn(*kind, rel)?;
                if new_insn.byte_len() != insn.byte_len() {
                    changed = true;
                }
                *insn = new_insn;
            }
            insn_index += 1;
        }
        if !changed {
            let (_, labels) = item_offsets(items);
            let program_labels = program_labels_from_items(items)?;
            let insns = items
                .iter()
                .filter_map(|item| match item {
                    DecodedItem::Insn { insn, .. } => Some(insn.clone()),
                    DecodedItem::Label(_) => None,
                })
                .collect();
            return Ok((insns, labels, program_labels));
        }
    }
    invalid_data("jump label relaxation did not converge")
}

fn program_labels_from_items(items: &[DecodedItem]) -> io::Result<Vec<ProgramLabel>> {
    let mut labels = Vec::new();
    let mut offset = 0usize;
    for item in items {
        match item {
            DecodedItem::Label(label) => labels.push(ProgramLabel {
                name: label.clone(),
                offset: to_u32(offset as u64, "Program label offset")?,
            }),
            DecodedItem::Insn { insn, .. } => {
                offset += insn.byte_len();
            }
        }
    }
    Ok(labels)
}

fn item_offsets(items: &[DecodedItem]) -> (Vec<usize>, BTreeMap<String, usize>) {
    let mut offsets = Vec::new();
    let mut labels = BTreeMap::new();
    let mut offset = 0usize;
    for item in items {
        match item {
            DecodedItem::Label(label) => {
                labels.insert(label.clone(), offset);
            }
            DecodedItem::Insn { insn, .. } => {
                offsets.push(offset);
                offset += insn.byte_len();
            }
        }
    }
    (offsets, labels)
}

fn jump_insn(kind: JumpKind, rel: i64) -> io::Result<Insn> {
    let rel_i32 = i32::try_from(rel)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "jump offset out of i32 range"))?;
    Ok(match kind {
        JumpKind::Jmp if i8::try_from(rel_i32).is_ok() => Insn::JmpShort(rel_i32 as i8),
        JumpKind::Jmp if i16::try_from(rel_i32).is_ok() => Insn::Jmp(rel_i32 as i16),
        JumpKind::Jmp => Insn::JmpWide(rel_i32),
        JumpKind::Jeqz if i8::try_from(rel_i32).is_ok() => Insn::JeqzShort(rel_i32 as i8),
        JumpKind::Jeqz if i16::try_from(rel_i32).is_ok() => Insn::Jeqz(rel_i32 as i16),
        JumpKind::Jeqz => Insn::JeqzWide(rel_i32),
        JumpKind::Jnez if i8::try_from(rel_i32).is_ok() => Insn::JnezShort(rel_i32 as i8),
        JumpKind::Jnez if i16::try_from(rel_i32).is_ok() => Insn::Jnez(rel_i32 as i16),
        JumpKind::Jnez => Insn::JnezWide(rel_i32),
    })
}

fn catch_entry_from_proto(
    catch: &crate::proto_panda::CatchBlock,
    labels: &BTreeMap<String, usize>,
) -> io::Result<CatchEntry> {
    Ok(CatchEntry {
        try_begin: label_offset(labels, &catch.try_begin_label)?,
        try_end: label_offset(labels, &catch.try_end_label)?,
        catch_begin: label_offset(labels, &catch.catch_begin_label)?,
        catch_end: label_offset(labels, &catch.catch_end_label)?,
    })
}

fn insn_from_proto(
    ins: &Ins,
    record_name: &str,
    strings: &mut StringTable,
    literal_key_to_index: &BTreeMap<String, usize>,
    refs: &mut MethodDecodeRefs,
) -> io::Result<DecodedItem> {
    let mut jump = None;
    let insn = match program_opcode_alias(ins.opcode) {
        Some(opcode) => match opcode {
            2 => Insn::LdUndefined,
            3 => Insn::LdNull,
            5 => Insn::LdGlobal,
            6 => Insn::LdTrue,
            7 => Insn::LdFalse,
            8 => Insn::LdHole,
            11 => Insn::LdThis,
            12 => Insn::PopLexEnv,
            15 => Insn::AsyncFunctionEnter,
            17 => Insn::Debugger,
            18 => Insn::GetPropIterator,
            19 => Insn::GetIterator(imm_u8(ins, 0, "getiterator slot")?),
            20 => Insn::CloseIterator(imm_u8(ins, 0, "closeiterator slot")?, reg_u8(ins, 0)?),
            23 => Insn::LdPrivateProperty(
                imm_u8(ins, 0, "ldprivateproperty ic")?,
                imm_u16(ins, 1, "ldprivateproperty level")?,
                imm_u16(ins, 2, "ldprivateproperty slot")?,
            ),
            24 => Insn::StPrivateProperty(
                imm_u8(ins, 0, "stprivateproperty ic")?,
                imm_u16(ins, 1, "stprivateproperty level")?,
                imm_u16(ins, 2, "stprivateproperty slot")?,
                reg_u8(ins, 0)?,
            ),
            25 => Insn::TestIn(
                imm_u8(ins, 0, "testin ic")?,
                imm_u16(ins, 1, "testin level")?,
                imm_u16(ins, 2, "testin slot")?,
            ),
            26 => Insn::DefineFieldByName(
                imm_u8(ins, 0, "definefieldbyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
                reg_u8(ins, 0)?,
            ),
            27 => Insn::DefinePropertyByName(
                imm_u8(ins, 0, "definepropertybyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
                reg_u8(ins, 0)?,
            ),
            28 => Insn::CreateEmptyObject,
            29 => Insn::CreateEmptyArray(imm_u16(ins, 0, "createemptyarray slot")?),
            30 => Insn::CreateGeneratorObj(reg_u8(ins, 0)?),
            32 | 33 => Insn::CreateObjectWithExcludedKeys(
                imm_u16(ins, 0, "createobjectwithexcludedkeys count")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            34 => {
                let literal = literal_ref(literal_key_to_index, id(ins, 0)?, refs)?;
                Insn::CreateArrayWithBuffer(imm_u8(ins, 0, "createarraywithbuffer slot")?, literal)
            }
            36 => {
                let literal = literal_ref(literal_key_to_index, id(ins, 0)?, refs)?;
                Insn::CreateObjectWithBuffer(
                    imm_u8(ins, 0, "createobjectwithbuffer slot")?,
                    literal,
                )
            }
            39 => Insn::NewObjApply(imm_u8(ins, 0, "newobjapply slot")?, reg_u8(ins, 0)?),
            40 => Insn::NewObjRange(
                imm_u8(ins, 0, "newobjrange slot")?,
                imm_u8(ins, 1, "newobjrange argc")?,
                reg_u8(ins, 0)?,
            ),
            42 => Insn::NewLexEnv(imm_u8(ins, 0, "newlexenv count")?),
            44 => {
                let literal = literal_ref(literal_key_to_index, id(ins, 0)?, refs)?;
                Insn::NewLexEnvWithName(imm_u8(ins, 0, "newlexenvwithname count")?, literal)
            }
            48 => Insn::Add2(imm_u8(ins, 0, "add2 slot")?, reg_u8(ins, 0)?),
            49 => Insn::Sub2(imm_u8(ins, 0, "sub2 slot")?, reg_u8(ins, 0)?),
            50 => Insn::Mul2(imm_u8(ins, 0, "mul2 slot")?, reg_u8(ins, 0)?),
            51 => Insn::Div2(imm_u8(ins, 0, "div2 slot")?, reg_u8(ins, 0)?),
            52 => Insn::Mod2(imm_u8(ins, 0, "mod2 slot")?, reg_u8(ins, 0)?),
            53 => Insn::Eq(imm_u8(ins, 0, "eq slot")?, reg_u8(ins, 0)?),
            54 => Insn::NotEq(imm_u8(ins, 0, "noteq slot")?, reg_u8(ins, 0)?),
            55 => Insn::Less(imm_u8(ins, 0, "less slot")?, reg_u8(ins, 0)?),
            56 => Insn::LessEq(imm_u8(ins, 0, "lesseq slot")?, reg_u8(ins, 0)?),
            57 => Insn::Greater(imm_u8(ins, 0, "greater slot")?, reg_u8(ins, 0)?),
            58 => Insn::GreaterEq(imm_u8(ins, 0, "greatereq slot")?, reg_u8(ins, 0)?),
            59 => Insn::Shl2(imm_u8(ins, 0, "shl2 slot")?, reg_u8(ins, 0)?),
            60 => Insn::Shr2(imm_u8(ins, 0, "shr2 slot")?, reg_u8(ins, 0)?),
            61 => Insn::Ashr2(imm_u8(ins, 0, "ashr2 slot")?, reg_u8(ins, 0)?),
            62 => Insn::And2(imm_u8(ins, 0, "and2 slot")?, reg_u8(ins, 0)?),
            63 => Insn::Or2(imm_u8(ins, 0, "or2 slot")?, reg_u8(ins, 0)?),
            64 => Insn::Xor2(imm_u8(ins, 0, "xor2 slot")?, reg_u8(ins, 0)?),
            65 => Insn::Exp(imm_u8(ins, 0, "exp slot")?, reg_u8(ins, 0)?),
            66 => Insn::TypeOf(imm_u8(ins, 0, "typeof slot")?),
            67 => Insn::ToNumber(imm_u8(ins, 0, "tonumber slot")?),
            69 => Insn::ToNumeric(imm_u8(ins, 0, "tonumeric slot")?),
            71 => Insn::Neg(imm_u8(ins, 0, "neg slot")?),
            73 => Insn::BitNot(imm_u8(ins, 0, "bitnot slot")?),
            75 => Insn::Inc(imm_u8(ins, 0, "inc slot")?),
            77 => Insn::Dec(imm_u8(ins, 0, "dec slot")?),
            79 => Insn::IsTrue,
            80 => Insn::IsFalse,
            81 => Insn::IsIn(imm_u8(ins, 0, "isin slot")?, reg_u8(ins, 0)?),
            82 => Insn::InstanceOf(imm_u8(ins, 0, "instanceof slot")?, reg_u8(ins, 0)?),
            83 => Insn::StrictNotEq(imm_u8(ins, 0, "strictnoteq slot")?, reg_u8(ins, 0)?),
            84 => Insn::StrictEq(imm_u8(ins, 0, "stricteq slot")?, reg_u8(ins, 0)?),
            86 => Insn::CallRuntimeDefineFieldByValue(
                imm_u8(ins, 0, "definefieldbyvalue slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            89 => {
                let literal = literal_ref(literal_key_to_index, id(ins, 0)?, refs)?;
                Insn::CallRuntimeCreatePrivateProperty(
                    imm_u16(ins, 0, "createprivateproperty count")?,
                    literal,
                )
            }
            90 => Insn::CallRuntimeDefinePrivateProperty(
                imm_u8(ins, 0, "defineprivateproperty ic")?,
                imm_u16(ins, 1, "defineprivateproperty level")?,
                imm_u16(ins, 2, "defineprivateproperty slot")?,
                reg_u8(ins, 0)?,
            ),
            91 => Insn::CallRuntimeCallInit(imm_u8(ins, 0, "callinit slot")?, reg_u8(ins, 0)?),
            102 => Insn::CallRuntimeIsTrue(imm_u8(ins, 0, "callruntime.istrue slot")?),
            103 => Insn::CallRuntimeIsFalse(imm_u8(ins, 0, "callruntime.isfalse slot")?),
            109 => Insn::Throw,
            111 => Insn::ThrowPatternNonCoercible,
            114 => Insn::ThrowIfNotObject(reg_u8(ins, 0)?),
            116 => {
                Insn::ThrowIfSuperNotCorrectCall(imm_u8(ins, 0, "throwifsupernotcorrectcall kind")?)
            }
            117 => Insn::ThrowUndefinedIfHoleWithName(intern_id(strings, id(ins, 0)?)?),
            120 => Insn::CallArg0(imm_u8(ins, 0, "callarg0 slot")?),
            122 => Insn::CallArg1(imm_u8(ins, 0, "callarg1 slot")?, reg_u8(ins, 0)?),
            124 => Insn::CallArgs2(
                imm_u8(ins, 0, "callargs2 slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            126 => Insn::CallArgs3(
                imm_u8(ins, 0, "callargs3 slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
                reg_u8(ins, 2)?,
            ),
            128 => Insn::CallRange(
                imm_u8(ins, 0, "callrange slot")?,
                imm_u8(ins, 1, "callrange argc")?,
                reg_u8(ins, 0)?,
            ),
            131 => Insn::SuperCallSpread(imm_u8(ins, 0, "supercallspread slot")?, reg_u8(ins, 0)?),
            132 => Insn::Apply(
                imm_u8(ins, 0, "apply slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            134 => Insn::CallThis0(imm_u8(ins, 0, "callthis0 slot")?, reg_u8(ins, 0)?),
            135 => Insn::CallThis1(
                imm_u8(ins, 0, "callthis1 slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            136 => Insn::CallThis2(
                imm_u8(ins, 0, "callthis2 slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
                reg_u8(ins, 2)?,
            ),
            137 => Insn::CallThis3(
                imm_u8(ins, 0, "callthis3 slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
                reg_u8(ins, 2)?,
                reg_u8(ins, 3)?,
            ),
            138 => Insn::CallThisRange(
                imm_u8(ins, 0, "callthisrange slot")?,
                imm_u8(ins, 1, "callthisrange argc")?,
                reg_u8(ins, 0)?,
            ),
            141 => Insn::SuperCallThisRange(
                imm_u8(ins, 0, "supercallthisrange slot")?,
                imm_u8(ins, 1, "supercallthisrange argc")?,
                reg_u8(ins, 0)?,
            ),
            143 => Insn::SuperCallArrowRange(
                imm_u8(ins, 0, "supercallarrowrange slot")?,
                imm_u8(ins, 1, "supercallarrowrange argc")?,
                reg_u8(ins, 0)?,
            ),
            145 => Insn::DefineGetterSetterByValue(
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
                reg_u8(ins, 2)?,
                reg_u8(ins, 3)?,
            ),
            146 => {
                let name = short_method_name(record_name, &utf8(id(ins, 0)?)?);
                let name_id = intern_id(strings, name.as_bytes())?;
                refs.definefunc_order.push(name.clone());
                Insn::DefineFuncShort(
                    imm_u8(ins, 0, "definefunc slot")?,
                    name_id,
                    imm_u8(ins, 1, "definefunc length")?,
                )
            }
            147 => {
                let name = short_method_name(record_name, &utf8(id(ins, 0)?)?);
                refs.method_pool_entries.insert(name.clone());
                Insn::DefineMethodShort(
                    imm_u8(ins, 0, "definemethod slot")?,
                    intern_id(strings, name.as_bytes())?,
                    imm_u8(ins, 1, "definemethod length")?,
                )
            }
            148 => {
                let ctor = short_method_name(record_name, &utf8(id(ins, 0)?)?);
                refs.constructor_names.insert(ctor.clone());
                refs.method_pool_entries.insert(ctor);
                let literal = literal_ref(literal_key_to_index, id(ins, 1)?, refs)?;
                Insn::DefineClassWithBuffer(
                    imm_u8(ins, 0, "defineclass slot")?,
                    0,
                    literal,
                    imm_u16(ins, 1, "defineclass argc")?,
                    reg_u8(ins, 0)?,
                )
            }
            150 => Insn::ResumeGenerator,
            152 => Insn::GetResumeMode,
            154 => Insn::GetTemplateObject(imm_u16(ins, 0, "gettemplateobject slot")?),
            156 => Insn::GetNextPropName(reg_u8(ins, 0)?),
            157 => Insn::DelObjProp(reg_u8(ins, 0)?),
            159 => Insn::SuspendGenerator(reg_u8(ins, 0)?),
            161 => Insn::AsyncFunctionAwaitUncaught(reg_u8(ins, 0)?),
            163 => Insn::CopyDataProperties(reg_u8(ins, 0)?),
            165 => Insn::StArraySpread(reg_u8(ins, 0)?, reg_u8(ins, 1)?),
            166 => Insn::SetObjectWithProto(
                imm_u8(ins, 0, "setobjectwithproto slot")?,
                reg_u8(ins, 0)?,
            ),
            168 => Insn::LdObjByValue(imm_u8(ins, 0, "ldobjbyvalue slot")?, reg_u8(ins, 0)?),
            170 => Insn::StObjByValue(
                imm_u8(ins, 0, "stobjbyvalue slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            171 => Insn::StOwnByValue(
                imm_u8(ins, 0, "stownbyvalue slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            172 => Insn::LdSuperByValue(imm_u8(ins, 0, "ldsuperbyvalue slot")?, reg_u8(ins, 0)?),
            174 => Insn::StSuperByValue(
                imm_u8(ins, 0, "stsuperbyvalue slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            180 | 181 => Insn::StOwnByIndex(
                imm_u16(ins, 0, "stownbyindex slot")?,
                reg_u8(ins, 0)?,
                imm_u16(ins, 1, "stownbyindex index")?,
            ),
            182 => Insn::AsyncFunctionResolve(reg_u8(ins, 0)?),
            184 => Insn::AsyncFunctionReject(reg_u8(ins, 0)?),
            186 => Insn::CopyRestArgs(imm_u8(ins, 0, "copyrestargs count")?),
            188 | 189 => Insn::LdLexVar(
                imm_u8(ins, 0, "ldlexvar level")?,
                imm_u8(ins, 1, "ldlexvar slot")?,
            ),
            190 | 191 => Insn::StLexVar(
                imm_u8(ins, 0, "stlexvar level")?,
                imm_u8(ins, 1, "stlexvar slot")?,
            ),
            196 => Insn::StModuleVar(imm_u8(ins, 0, "stmodulevar index")?),
            199 => Insn::TryLdGlobalByName(
                imm_u8(ins, 0, "tryldglobalbyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
            ),
            200 => Insn::TryStGlobalByName(
                imm_u8(ins, 0, "trystglobalbyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
            ),
            203 => Insn::LdObjByName(
                imm_u8(ins, 0, "ldobjbyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
            ),
            205 => Insn::StObjByName(
                imm_u8(ins, 0, "stobjbyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
                reg_u8(ins, 0)?,
            ),
            206 => Insn::StOwnByName(
                imm_u8(ins, 0, "stownbyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
                reg_u8(ins, 0)?,
            ),
            207 => Insn::LdSuperByName(
                imm_u8(ins, 0, "ldsuperbyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
            ),
            209 => Insn::StSuperByName(
                imm_u8(ins, 0, "stsuperbyname slot")?,
                intern_id(strings, id(ins, 0)?)?,
                reg_u8(ins, 0)?,
            ),
            210 => Insn::LdLocalModuleVar(imm_u8(ins, 0, "ldlocalmodulevar index")?),
            212 => Insn::LdExternalModuleVar(imm_u8(ins, 0, "ldexternalmodulevar index")?),
            222 => Insn::StOwnByValueWithNameSet(
                imm_u8(ins, 0, "stownbyvaluewithnameset slot")?,
                reg_u8(ins, 0)?,
                reg_u8(ins, 1)?,
            ),
            223 => Insn::StOwnByNameWithNameSet(
                imm_u8(ins, 0, "stownbynamewithnameset slot")?,
                intern_id(strings, id(ins, 0)?)?,
                reg_u8(ins, 0)?,
            ),
            224 => Insn::LdBigInt(intern_id(strings, id(ins, 0)?)?),
            231 => Insn::DynamicImport,
            236 => Insn::LdaStr(intern_id(strings, id(ins, 0)?)?),
            237 => {
                let label = utf8(id(ins, 0)?)?;
                jump = Some((JumpKind::Jmp, label));
                Insn::JmpWide(0)
            }
            238 => {
                let label = utf8(id(ins, 0)?)?;
                jump = Some((JumpKind::Jeqz, label));
                Insn::JeqzWide(0)
            }
            239 => {
                let label = utf8(id(ins, 0)?)?;
                jump = Some((JumpKind::Jnez, label));
                Insn::JnezWide(0)
            }
            254 => {
                let dst = reg_u16(ins, 0)?;
                let src = reg_u16(ins, 1)?;
                if dst <= 0x0f && src <= 0x0f {
                    Insn::MovV4(dst as u8, src as u8)
                } else if dst <= u8::MAX as u16 && src <= u8::MAX as u16 {
                    Insn::MovV8(dst as u8, src as u8)
                } else {
                    Insn::MovV16(dst, src)
                }
            }
            255 => Insn::Lda(reg_u8(ins, 0)?),
            256 => Insn::Sta(reg_u8(ins, 0)?),
            257 => Insn::LdAi(imm_i32(ins, 0, "ldai value")?),
            258 => Insn::FLdAi(read_imm_f64(ins, 0, "fldai value")?),
            259 => Insn::Return,
            260 => Insn::ReturnUndefined,
            261 => Insn::Nop,
            _ => unreachable!("program opcode alias must map to a handled branch"),
        },
        None => generic_insn_from_proto(
            ins,
            ins.opcode,
            record_name,
            strings,
            literal_key_to_index,
            refs,
        )?,
    };
    Ok(DecodedItem::Insn { insn, jump })
}

fn program_opcode_alias(opcode: u32) -> Option<u32> {
    macro_rules! alias {
        ($dispatch:literal, $insn:expr) => {
            if opcode == pandasm_opcode(&$insn) {
                return Some($dispatch);
            }
        };
    }

    alias!(2, Insn::LdUndefined);
    alias!(3, Insn::LdNull);
    alias!(5, Insn::LdGlobal);
    alias!(6, Insn::LdTrue);
    alias!(7, Insn::LdFalse);
    alias!(8, Insn::LdHole);
    alias!(11, Insn::LdThis);
    alias!(12, Insn::PopLexEnv);
    alias!(15, Insn::AsyncFunctionEnter);
    alias!(17, Insn::Debugger);
    alias!(18, Insn::GetPropIterator);
    alias!(19, Insn::GetIterator(0));
    alias!(20, Insn::CloseIterator(0, 0));
    alias!(23, Insn::LdPrivateProperty(0, 0, 0));
    alias!(24, Insn::StPrivateProperty(0, 0, 0, 0));
    alias!(25, Insn::TestIn(0, 0, 0));
    alias!(26, Insn::DefineFieldByName(0, 0, 0));
    alias!(27, Insn::DefinePropertyByName(0, 0, 0));
    alias!(28, Insn::CreateEmptyObject);
    alias!(29, Insn::CreateEmptyArray(0));
    alias!(29, Insn::CreateEmptyArray(u8::MAX as u16 + 1));
    alias!(30, Insn::CreateGeneratorObj(0));
    alias!(32, Insn::CreateObjectWithExcludedKeys(0, 0, 0));
    alias!(
        33,
        Insn::CreateObjectWithExcludedKeys(u8::MAX as u16 + 1, 0, 0)
    );
    alias!(34, Insn::CreateArrayWithBuffer(0, 0));
    alias!(36, Insn::CreateObjectWithBuffer(0, 0));
    alias!(39, Insn::NewObjApply(0, 0));
    alias!(40, Insn::NewObjRange(0, 0, 0));
    alias!(42, Insn::NewLexEnv(0));
    alias!(44, Insn::NewLexEnvWithName(0, 0));
    alias!(48, Insn::Add2(0, 0));
    alias!(49, Insn::Sub2(0, 0));
    alias!(50, Insn::Mul2(0, 0));
    alias!(51, Insn::Div2(0, 0));
    alias!(52, Insn::Mod2(0, 0));
    alias!(53, Insn::Eq(0, 0));
    alias!(54, Insn::NotEq(0, 0));
    alias!(55, Insn::Less(0, 0));
    alias!(56, Insn::LessEq(0, 0));
    alias!(57, Insn::Greater(0, 0));
    alias!(58, Insn::GreaterEq(0, 0));
    alias!(59, Insn::Shl2(0, 0));
    alias!(60, Insn::Shr2(0, 0));
    alias!(61, Insn::Ashr2(0, 0));
    alias!(62, Insn::And2(0, 0));
    alias!(63, Insn::Or2(0, 0));
    alias!(64, Insn::Xor2(0, 0));
    alias!(65, Insn::Exp(0, 0));
    alias!(66, Insn::TypeOf(0));
    alias!(67, Insn::ToNumber(0));
    alias!(69, Insn::ToNumeric(0));
    alias!(71, Insn::Neg(0));
    alias!(73, Insn::BitNot(0));
    alias!(75, Insn::Inc(0));
    alias!(77, Insn::Dec(0));
    alias!(79, Insn::IsTrue);
    alias!(80, Insn::IsFalse);
    alias!(81, Insn::IsIn(0, 0));
    alias!(82, Insn::InstanceOf(0, 0));
    alias!(83, Insn::StrictNotEq(0, 0));
    alias!(84, Insn::StrictEq(0, 0));
    alias!(86, Insn::CallRuntimeDefineFieldByValue(0, 0, 0));
    alias!(89, Insn::CallRuntimeCreatePrivateProperty(0, 0));
    alias!(90, Insn::CallRuntimeDefinePrivateProperty(0, 0, 0, 0));
    alias!(91, Insn::CallRuntimeCallInit(0, 0));
    alias!(102, Insn::CallRuntimeIsTrue(0));
    alias!(103, Insn::CallRuntimeIsFalse(0));
    alias!(109, Insn::Throw);
    alias!(111, Insn::ThrowPatternNonCoercible);
    alias!(114, Insn::ThrowIfNotObject(0));
    alias!(116, Insn::ThrowIfSuperNotCorrectCall(0));
    alias!(117, Insn::ThrowUndefinedIfHoleWithName(0));
    alias!(120, Insn::CallArg0(0));
    alias!(122, Insn::CallArg1(0, 0));
    alias!(124, Insn::CallArgs2(0, 0, 0));
    alias!(126, Insn::CallArgs3(0, 0, 0, 0));
    alias!(128, Insn::CallRange(0, 0, 0));
    alias!(131, Insn::SuperCallSpread(0, 0));
    alias!(132, Insn::Apply(0, 0, 0));
    alias!(134, Insn::CallThis0(0, 0));
    alias!(135, Insn::CallThis1(0, 0, 0));
    alias!(136, Insn::CallThis2(0, 0, 0, 0));
    alias!(137, Insn::CallThis3(0, 0, 0, 0, 0));
    alias!(138, Insn::CallThisRange(0, 0, 0));
    alias!(141, Insn::SuperCallThisRange(0, 0, 0));
    alias!(143, Insn::SuperCallArrowRange(0, 0, 0));
    alias!(145, Insn::DefineGetterSetterByValue(0, 0, 0, 0));
    alias!(146, Insn::DefineFuncShort(0, 0, 0));
    alias!(147, Insn::DefineMethodShort(0, 0, 0));
    alias!(148, Insn::DefineClassWithBuffer(0, 0, 0, 0, 0));
    alias!(150, Insn::ResumeGenerator);
    alias!(152, Insn::GetResumeMode);
    alias!(154, Insn::GetTemplateObject(0));
    alias!(154, Insn::GetTemplateObject(u8::MAX as u16 + 1));
    alias!(156, Insn::GetNextPropName(0));
    alias!(157, Insn::DelObjProp(0));
    alias!(159, Insn::SuspendGenerator(0));
    alias!(161, Insn::AsyncFunctionAwaitUncaught(0));
    alias!(163, Insn::CopyDataProperties(0));
    alias!(165, Insn::StArraySpread(0, 0));
    alias!(166, Insn::SetObjectWithProto(0, 0));
    alias!(168, Insn::LdObjByValue(0, 0));
    alias!(170, Insn::StObjByValue(0, 0, 0));
    alias!(171, Insn::StOwnByValue(0, 0, 0));
    alias!(172, Insn::LdSuperByValue(0, 0));
    alias!(174, Insn::StSuperByValue(0, 0, 0));
    alias!(180, Insn::StOwnByIndex(0, 0, 0));
    alias!(181, Insn::StOwnByIndex(u8::MAX as u16 + 1, 0, 0));
    alias!(182, Insn::AsyncFunctionResolve(0));
    alias!(184, Insn::AsyncFunctionReject(0));
    alias!(186, Insn::CopyRestArgs(0));
    alias!(188, Insn::LdLexVar(0, 0));
    alias!(189, Insn::LdLexVar(0x10, 0));
    alias!(190, Insn::StLexVar(0, 0));
    alias!(191, Insn::StLexVar(0x10, 0));
    alias!(196, Insn::StModuleVar(0));
    alias!(199, Insn::TryLdGlobalByName(0, 0));
    alias!(200, Insn::TryStGlobalByName(0, 0));
    alias!(203, Insn::LdObjByName(0, 0));
    alias!(205, Insn::StObjByName(0, 0, 0));
    alias!(206, Insn::StOwnByName(0, 0, 0));
    alias!(207, Insn::LdSuperByName(0, 0));
    alias!(209, Insn::StSuperByName(0, 0, 0));
    alias!(210, Insn::LdLocalModuleVar(0));
    alias!(212, Insn::LdExternalModuleVar(0));
    alias!(222, Insn::StOwnByValueWithNameSet(0, 0, 0));
    alias!(223, Insn::StOwnByNameWithNameSet(0, 0, 0));
    alias!(224, Insn::LdBigInt(0));
    alias!(231, Insn::DynamicImport);
    alias!(236, Insn::LdaStr(0));
    alias!(237, Insn::JmpShort(0));
    alias!(237, Insn::Jmp(0));
    alias!(237, Insn::JmpWide(0));
    alias!(238, Insn::JeqzShort(0));
    alias!(238, Insn::Jeqz(0));
    alias!(238, Insn::JeqzWide(0));
    alias!(239, Insn::JnezShort(0));
    alias!(239, Insn::Jnez(0));
    alias!(239, Insn::JnezWide(0));
    alias!(254, Insn::MovV4(0, 0));
    alias!(254, Insn::MovV8(0, 0));
    alias!(254, Insn::MovV16(0, 0));
    alias!(255, Insn::Lda(0));
    alias!(256, Insn::Sta(0));
    alias!(257, Insn::LdAi(0));
    alias!(258, Insn::FLdAi(0.0));
    alias!(259, Insn::Return);
    alias!(260, Insn::ReturnUndefined);
    alias!(261, Insn::Nop);
    None
}

fn generic_insn_from_proto(
    ins: &Ins,
    opcode: u32,
    record_name: &str,
    strings: &mut StringTable,
    literal_key_to_index: &BTreeMap<String, usize>,
    refs: &mut MethodDecodeRefs,
) -> io::Result<Insn> {
    let imms = ins
        .imms
        .iter()
        .map(|imm| match imm.r#type.as_ref() {
            Some(ins::i_type::Type::ValueInt(value)) => Ok(PandasmImm::Int(*value)),
            Some(ins::i_type::Type::ValueDouble(value)) => Ok(PandasmImm::Double(*value)),
            None => invalid_data("missing generic pandasm immediate"),
        })
        .collect::<io::Result<Vec<_>>>()?;
    let bytes = encode_pandasm_instruction(opcode, &ins.regs, &ins.ids, &imms, |kind, id| {
        resolve_pandasm_proto_id(kind, id, record_name, strings, literal_key_to_index, refs)
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported pandasm opcode {opcode}"),
        )
    })?;
    Ok(Insn::Pandasm(PandasmInsn {
        opcode,
        regs: ins.regs.clone(),
        ids: ins.ids.clone(),
        imms,
        bytes,
    }))
}

fn resolve_pandasm_proto_id(
    kind: IdKind,
    id: &[u8],
    record_name: &str,
    strings: &mut StringTable,
    literal_key_to_index: &BTreeMap<String, usize>,
    refs: &mut MethodDecodeRefs,
) -> Option<u32> {
    match kind {
        IdKind::String => intern_id(strings, id).ok().map(u32::from),
        IdKind::Method => {
            let name = std::str::from_utf8(id).ok()?;
            let name = short_method_name(record_name, name);
            intern_id(strings, name.as_bytes()).ok().map(u32::from)
        }
        IdKind::LiteralArray => literal_ref(literal_key_to_index, id, refs)
            .ok()
            .map(u32::from),
        IdKind::Other => None,
    }
}

fn method_kind_from_proto(name: &str, access_flags: u32, function_kind: u32) -> MethodKind {
    if name == "func_main_0" {
        MethodKind::Module
    } else if access_flags == method_access::ANON {
        MethodKind::Anonymous
    } else if access_flags == method_access::MODULE_OR_CTOR || function_kind == 2 {
        MethodKind::FunctionLiteral
    } else {
        MethodKind::Function
    }
}

fn record_access_flags(record: &Record) -> u32 {
    record
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.father.as_ref())
        .map(|metadata| metadata.access_flags)
        .unwrap_or(1)
}

fn field_access_flags(field: &Field) -> u32 {
    field
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.father.as_ref())
        .map(|metadata| metadata.access_flags)
        .unwrap_or(0)
}

fn function_access_flags(function: &Function) -> u32 {
    function
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.father.as_ref())
        .map(|metadata| metadata.access_flags)
        .unwrap_or(method_access::INSTANCE)
}

fn function_expected_property_count(function: &Function) -> io::Result<Option<u32>> {
    function_annotation_u32(
        function,
        EXPECTED_PROPERTY_COUNT_RECORD_NAME,
        "ExpectedPropertyCount",
    )
}

fn function_annotation_u32(
    function: &Function,
    record_name: &str,
    element_name: &str,
) -> io::Result<Option<u32>> {
    let Some(annotation_metadata) = function
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.father.as_ref())
        .and_then(|metadata| metadata.father.as_ref())
    else {
        return Ok(None);
    };

    for annotation in &annotation_metadata.annotations {
        if annotation.record_name != record_name.as_bytes() {
            continue;
        }
        for element in &annotation.elements {
            if element.name != element_name.as_bytes() {
                continue;
            }
            let Some(annotation_element::Value::Scalar(value)) = &element.value else {
                return invalid_data(format!(
                    "annotation {record_name}.{element_name} is not scalar"
                ));
            };
            return Ok(Some(to_u32(
                scalar_u64_value(value)?,
                "function annotation u32",
            )?));
        }
    }

    Ok(None)
}

fn type_idx(value: &Type) -> u16 {
    match std::str::from_utf8(&value.name).ok() {
        Some("u8") => 0,
        Some("u32") => 1,
        _ => 0,
    }
}

fn scalar_u64_value(value: &ScalarValue) -> io::Result<u64> {
    match &value.value {
        Some(scalar_value::Value::ValueU64(value)) => Ok(*value),
        _ => invalid_data("missing scalar uint64 value"),
    }
}

fn scalar_string_value(value: &ScalarValue) -> io::Result<String> {
    match &value.value {
        Some(scalar_value::Value::ValueStr(value)) => utf8(value),
        _ => invalid_data("missing scalar string value"),
    }
}

fn label_offset(labels: &BTreeMap<String, usize>, label: &[u8]) -> io::Result<u32> {
    let label = utf8(label)?;
    let Some(offset) = labels.get(&label) else {
        return invalid_data(format!("missing catch label {label}"));
    };
    to_u32(*offset as u64, "catch label offset")
}

fn literal_ref(
    literal_key_to_index: &BTreeMap<String, usize>,
    key: &[u8],
    refs: &mut MethodDecodeRefs,
) -> io::Result<u16> {
    let key = utf8(key)?;
    let Some(&index) = literal_key_to_index.get(&key) else {
        return invalid_data(format!("missing literal array key {key}"));
    };
    refs.literal_indices.insert(index);
    to_u16(index as u64, "literal array index")
}

fn intern_id(strings: &mut StringTable, value: &[u8]) -> io::Result<u16> {
    let value = utf8(value)?;
    let index = strings.intern(&value);
    to_u16(index as u64, "string index")
}

fn id(ins: &Ins, index: usize) -> io::Result<&[u8]> {
    ins.ids
        .get(index)
        .map(Vec::as_slice)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing instruction id"))
}

fn reg_u8(ins: &Ins, index: usize) -> io::Result<u8> {
    let value = *ins.regs.get(index).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing instruction register")
    })?;
    to_u8(value as u64, "instruction register")
}

fn reg_u16(ins: &Ins, index: usize) -> io::Result<u16> {
    let value = *ins.regs.get(index).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing instruction register")
    })?;
    to_u16(value as u64, "instruction register")
}

fn read_imm_i64(ins: &Ins, index: usize, name: &str) -> io::Result<i64> {
    match ins.imms.get(index).and_then(|value| value.r#type.as_ref()) {
        Some(ins::i_type::Type::ValueInt(value)) => Ok(*value),
        _ => invalid_data(format!("missing integer immediate {name}")),
    }
}

fn imm_i32(ins: &Ins, index: usize, name: &str) -> io::Result<i32> {
    let value = read_imm_i64(ins, index, name)?;
    i32::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} out of i32 range"),
        )
    })
}

fn imm_u8(ins: &Ins, index: usize, name: &str) -> io::Result<u8> {
    let value = read_imm_i64(ins, index, name)?;
    if value < 0 {
        return invalid_data(format!("{name} is negative"));
    }
    to_u8(value as u64, name)
}

fn imm_u16(ins: &Ins, index: usize, name: &str) -> io::Result<u16> {
    let value = read_imm_i64(ins, index, name)?;
    if value < 0 {
        return invalid_data(format!("{name} is negative"));
    }
    to_u16(value as u64, name)
}

fn read_imm_f64(ins: &Ins, index: usize, name: &str) -> io::Result<f64> {
    match ins.imms.get(index).and_then(|value| value.r#type.as_ref()) {
        Some(ins::i_type::Type::ValueDouble(value)) => Ok(*value),
        _ => invalid_data(format!("missing double immediate {name}")),
    }
}

fn utf8(value: &[u8]) -> io::Result<String> {
    String::from_utf8(value.to_vec()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid protoPanda UTF-8 string: {error}"),
        )
    })
}

fn to_u8(value: u64, name: &str) -> io::Result<u8> {
    u8::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} out of u8 range"),
        )
    })
}

fn to_u16(value: u64, name: &str) -> io::Result<u16> {
    u16::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} out of u16 range"),
        )
    })
}

fn to_u32(value: u64, name: &str) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} out of u32 range"),
        )
    })
}

fn invalid_data<T>(message: impl Into<String>) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, message.into()))
}

fn record_to_proto(record: &ClassRecord, file: &AbcFile) -> Record {
    Record {
        name: bytes(&pandasm_record_name(&record.name)),
        conflict: false,
        language: record.source_lang as u32,
        metadata: Some(record_metadata(record.access_flags)),
        field_list: record
            .fields
            .iter()
            .map(|field| field_to_proto(field, record, file))
            .collect(),
        params_num: 0,
        body_presence: false,
        body_location: None,
        source_file: record.source_file.as_deref().map(bytes).unwrap_or_default(),
        file_location: None,
    }
}

fn field_to_proto(field: &FieldRecord, record: &ClassRecord, file: &AbcFile) -> Field {
    let field_type = field_type(field.type_idx);
    Field {
        r#type: Some(field_type.clone()),
        name: bytes(&field.name),
        metadata: Some(FieldMetadata {
            father: Some(item_metadata(field.access_flags)),
            field_type: Some(field_type),
            value: field
                .value
                .map(|value| field_scalar_value(value, record, file)),
        }),
        line_of_def: 0,
        whole_line: Vec::new(),
        bound_left: 0,
        bound_right: 0,
        is_defined: false,
    }
}

fn field_scalar_value(value: FieldValue, record: &ClassRecord, _file: &AbcFile) -> ScalarValue {
    match value {
        FieldValue::U8(value) => scalar_u64(VALUE_TYPE_U8, value as u64),
        FieldValue::U32(value) => scalar_u64(VALUE_TYPE_U32, value as u64),
        FieldValue::LiteralArray(index) => {
            scalar_string(VALUE_TYPE_LITERALARRAY, literal_key(record, index))
        }
    }
}

fn function_to_proto(
    method: &MethodRecord,
    record: &ClassRecord,
    strings: &StringTable,
) -> io::Result<Function> {
    let mut pandasm_insns = method.pandasm_insns.clone();
    let slots_num = assign_inline_cache_slots(&mut pandasm_insns);

    let mut required_label_offsets = BTreeSet::<usize>::new();
    let offsets = instruction_offsets(&pandasm_insns);
    for (idx, insn) in pandasm_insns.iter().enumerate() {
        if let Some(target) = jump_target(offsets[idx], insn) {
            required_label_offsets.insert(target);
        }
    }
    for catch in &method.catch_entries {
        required_label_offsets.insert(catch.try_begin as usize);
        required_label_offsets.insert(catch.try_end as usize);
        required_label_offsets.insert(catch.catch_begin as usize);
        required_label_offsets.insert(catch.catch_end as usize);
    }
    let label_plan = method_label_plan(&pandasm_insns, &required_label_offsets, method);

    let mut ins = Vec::with_capacity(method.pandasm_insns.len() + label_plan.len());
    for (idx, asm) in pandasm_insns.iter().enumerate() {
        let offset = offsets[idx];
        if let Some(names) = label_plan.names_at(offset) {
            for name in names {
                ins.push(label_ins(name));
            }
        }
        ins.push(insn_to_proto(
            asm,
            method.pandasm_debug.get(idx).copied(),
            offset,
            record,
            strings,
            label_plan.target_names(),
        )?);
    }
    let end_offset = pandasm_insns.iter().map(Insn::byte_len).sum::<usize>();
    if let Some(names) = label_plan.names_at(end_offset) {
        for name in names {
            ins.push(label_ins(name));
        }
    }

    Ok(Function {
        name: bytes(&full_method_name(&record.name, &method.name)),
        language: record.source_lang as u32,
        metadata: Some(function_metadata(
            method.access_flags(),
            method.expected_property_count,
            slots_num,
            method.function_kind,
            &method.concurrent_module_requests,
        )),
        label_table: Vec::new(),
        ins,
        local_variable_debug: Vec::new(),
        source_file: method
            .debug_info
            .as_ref()
            .map(|debug| bytes(&debug.source_file))
            .unwrap_or_default(),
        source_code: Vec::new(),
        catch_blocks: method
            .catch_entries
            .iter()
            .map(|catch| -> io::Result<crate::proto_panda::CatchBlock> {
                Ok(crate::proto_panda::CatchBlock {
                    whole_line: Vec::new(),
                    exception_record: Vec::new(),
                    try_begin_label: bytes(label_name_for_offset(
                        label_plan.target_names(),
                        catch.try_begin as usize,
                    )?),
                    try_end_label: bytes(label_name_for_offset(
                        label_plan.target_names(),
                        catch.try_end as usize,
                    )?),
                    catch_begin_label: bytes(label_name_for_offset(
                        label_plan.target_names(),
                        catch.catch_begin as usize,
                    )?),
                    catch_end_label: bytes(label_name_for_offset(
                        label_plan.target_names(),
                        catch.catch_end as usize,
                    )?),
                })
            })
            .collect::<io::Result<Vec<_>>>()?,
        value_of_first_param: -1,
        regs_num: method.num_vregs as u64,
        params: (0..method.num_args)
            .map(|_| Parameter {
                r#type: Some(type_named("any")),
                metadata: None,
            })
            .collect(),
        body_presence: !method.bytecode.is_empty(),
        return_type: Some(type_named("any")),
        body_location: Some(default_source_location()),
        file_location: None,
        function_kind: method.function_kind,
        slots_num: slots_num.into(),
        concurrent_module_requests: method.concurrent_module_requests.clone(),
    })
}

fn assign_inline_cache_slots(insns: &mut [Insn]) -> u32 {
    let mut current_slot = 0u32;
    for insn in insns {
        if let Some(slot_size) = insn.assign_inline_cache_slot(current_slot) {
            current_slot += slot_size as u32;
        }
    }
    current_slot
}

fn instruction_offsets(insns: &[Insn]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(insns.len());
    let mut offset = 0usize;
    for insn in insns {
        offsets.push(offset);
        offset += insn.byte_len();
    }
    offsets
}

struct FunctionLabelPlan {
    labels_by_offset: BTreeMap<usize, Vec<String>>,
    target_names: BTreeMap<usize, String>,
    len: usize,
}

impl FunctionLabelPlan {
    fn new() -> Self {
        Self {
            labels_by_offset: BTreeMap::new(),
            target_names: BTreeMap::new(),
            len: 0,
        }
    }

    fn add(&mut self, offset: usize, name: String) {
        self.labels_by_offset
            .entry(offset)
            .or_default()
            .push(name.clone());
        self.target_names.entry(offset).or_insert(name);
        self.len += 1;
    }

    fn names_at(&self, offset: usize) -> Option<&[String]> {
        self.labels_by_offset.get(&offset).map(Vec::as_slice)
    }

    fn target_names(&self) -> &BTreeMap<usize, String> {
        &self.target_names
    }

    fn len(&self) -> usize {
        self.len
    }
}

fn method_label_plan(
    insns: &[Insn],
    required_offsets: &BTreeSet<usize>,
    method: &MethodRecord,
) -> FunctionLabelPlan {
    let end_offset = insns.iter().map(Insn::byte_len).sum::<usize>();
    let mut plan = FunctionLabelPlan::new();
    let mut used_names = BTreeSet::<String>::new();

    if method.label_style == MethodLabelStyle::Source {
        for label in &method.program_labels {
            let offset = label.offset as usize;
            if offset > end_offset || !used_names.insert(label.name.clone()) {
                continue;
            }
            plan.add(offset, label.name.clone());
        }
    }

    let fallback_names = method_label_names(insns, required_offsets, method.label_style);
    for &offset in required_offsets {
        if plan.target_names.contains_key(&offset) {
            continue;
        }
        let base = fallback_names
            .get(&offset)
            .cloned()
            .unwrap_or_else(|| label_name_for_source_index(plan.len()));
        let name = unique_label_name(base, &mut used_names);
        plan.add(offset, name);
    }

    plan
}

fn unique_label_name(base: String, used_names: &mut BTreeSet<String>) -> String {
    if used_names.insert(base.clone()) {
        return base;
    }
    for suffix in 1usize.. {
        let candidate = format!("{base}_{suffix}");
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("usize label suffix space exhausted")
}

fn method_label_names(
    insns: &[Insn],
    label_offsets: &BTreeSet<usize>,
    style: MethodLabelStyle,
) -> BTreeMap<usize, String> {
    let offsets = instruction_offsets(insns);
    label_offsets
        .iter()
        .enumerate()
        .map(|(index, &offset)| {
            let name = match style {
                MethodLabelStyle::Source => label_name_for_source_index(index),
                MethodLabelStyle::Abc2Program => {
                    label_name_for_inst_idx(instruction_index_at_offset(&offsets, offset))
                }
            };
            (offset, name)
        })
        .collect()
}

fn instruction_index_at_offset(offsets: &[usize], offset: usize) -> usize {
    offsets
        .binary_search(&offset)
        .unwrap_or_else(|_| offsets.len())
}

fn label_name_for_offset(label_names: &BTreeMap<usize, String>, offset: usize) -> io::Result<&str> {
    label_names
        .get(&offset)
        .map(String::as_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Program label"))
}

fn jump_target(offset: usize, insn: &Insn) -> Option<usize> {
    let rel = match insn {
        Insn::JmpShort(value) => *value as i32,
        Insn::Jmp(value) => *value as i32,
        Insn::JmpWide(value) => *value,
        Insn::JeqzShort(value) => *value as i32,
        Insn::Jeqz(value) => *value as i32,
        Insn::JeqzWide(value) => *value,
        Insn::JnezShort(value) => *value as i32,
        Insn::Jnez(value) => *value as i32,
        Insn::JnezWide(value) => *value,
        _ => return None,
    };
    (offset as i64 + rel as i64).try_into().ok()
}

fn label_ins(name: &str) -> Ins {
    Ins {
        opcode: pandasm_invalid_opcode(),
        regs: Vec::new(),
        ids: Vec::new(),
        imms: Vec::new(),
        label: bytes(name),
        set_label_val: true,
        ins_debug: Some(default_ins_debug()),
    }
}

fn ins_debug(debug: Option<InstructionDebugInfo>) -> DebuginfoIns {
    let Some(debug) = debug else {
        return default_ins_debug();
    };
    DebuginfoIns {
        line_number: proto_debug_line(debug.line_number),
        column_number: debug.column_number,
        whole_line: Vec::new(),
        bound_left: 0,
        bound_right: 0,
    }
}

fn proto_debug_line(line_number: u32) -> u64 {
    if line_number == u32::MAX {
        u64::MAX
    } else {
        line_number as u64
    }
}

fn default_ins_debug() -> DebuginfoIns {
    DebuginfoIns {
        line_number: u64::MAX,
        column_number: 0,
        whole_line: Vec::new(),
        bound_left: 0,
        bound_right: 0,
    }
}

fn insn_to_proto(
    insn: &Insn,
    debug: Option<InstructionDebugInfo>,
    offset: usize,
    record: &ClassRecord,
    strings: &StringTable,
    label_names: &BTreeMap<usize, String>,
) -> io::Result<Ins> {
    let mut out = Ins {
        opcode: pandasm_opcode(insn),
        regs: Vec::new(),
        ids: Vec::new(),
        imms: Vec::new(),
        label: Vec::new(),
        set_label_val: false,
        ins_debug: Some(ins_debug(debug)),
    };

    if let Insn::Pandasm(raw) = insn {
        out.opcode = raw.opcode;
        out.regs = raw.regs.clone();
        out.ids = raw.ids.clone();
        out.imms = raw
            .imms
            .iter()
            .map(|imm| match imm {
                PandasmImm::Int(value) => imm_i64(*value),
                PandasmImm::Double(value) => imm_f64(*value),
            })
            .collect();
        return Ok(out);
    }

    match insn {
        Insn::Pandasm(_) => unreachable!("generic pandasm instruction serialized above"),
        Insn::LdUndefined
        | Insn::LdNull
        | Insn::LdTrue
        | Insn::LdFalse
        | Insn::LdHole
        | Insn::LdGlobal
        | Insn::DynamicImport
        | Insn::GetImportMeta
        | Insn::Debugger
        | Insn::LdThis
        | Insn::PopLexEnv
        | Insn::CreateEmptyObject
        | Insn::Return
        | Insn::ReturnUndefined
        | Insn::Throw
        | Insn::ThrowPatternNonCoercible
        | Insn::GetPropIterator
        | Insn::ResumeGenerator
        | Insn::GetResumeMode
        | Insn::AsyncFunctionEnter
        | Insn::Nop
        | Insn::IsTrue
        | Insn::IsFalse => {}
        Insn::LdAi(value) => out.imms.push(imm_i64(*value as i64)),
        Insn::FLdAi(value) => out.imms.push(imm_f64(*value)),
        Insn::LdaStr(id) | Insn::LdBigInt(id) => out.ids.push(string_id(strings, *id)?),
        Insn::Lda(reg)
        | Insn::Sta(reg)
        | Insn::ThrowIfNotObject(reg)
        | Insn::CopyDataProperties(reg)
        | Insn::DelObjProp(reg)
        | Insn::GetNextPropName(reg)
        | Insn::CreateGeneratorObj(reg)
        | Insn::SuspendGenerator(reg)
        | Insn::AsyncFunctionAwaitUncaught(reg)
        | Insn::AsyncFunctionResolve(reg)
        | Insn::AsyncFunctionReject(reg) => out.regs.push(*reg as u32),
        Insn::MovV4(dst, src) | Insn::MovV8(dst, src) => {
            out.regs.extend([*dst as u32, *src as u32]);
        }
        Insn::MovV16(dst, src) => out.regs.extend([*dst as u32, *src as u32]),
        Insn::Add2(slot, reg)
        | Insn::Sub2(slot, reg)
        | Insn::Mul2(slot, reg)
        | Insn::Div2(slot, reg)
        | Insn::Mod2(slot, reg)
        | Insn::Eq(slot, reg)
        | Insn::NotEq(slot, reg)
        | Insn::Less(slot, reg)
        | Insn::LessEq(slot, reg)
        | Insn::Greater(slot, reg)
        | Insn::GreaterEq(slot, reg)
        | Insn::Shl2(slot, reg)
        | Insn::Shr2(slot, reg)
        | Insn::Ashr2(slot, reg)
        | Insn::And2(slot, reg)
        | Insn::Or2(slot, reg)
        | Insn::Xor2(slot, reg)
        | Insn::Exp(slot, reg)
        | Insn::IsIn(slot, reg)
        | Insn::InstanceOf(slot, reg)
        | Insn::StrictNotEq(slot, reg)
        | Insn::StrictEq(slot, reg)
        | Insn::CallArg1(slot, reg)
        | Insn::CallThis0(slot, reg)
        | Insn::LdObjByValue(slot, reg)
        | Insn::LdSuperByValue(slot, reg)
        | Insn::SetObjectWithProto(slot, reg)
        | Insn::SuperCallSpread(slot, reg)
        | Insn::NewObjApply(slot, reg)
        | Insn::CloseIterator(slot, reg)
        | Insn::CallRuntimeCallInit(slot, reg) => {
            out.imms.push(imm_i64(*slot as i64));
            out.regs.push(*reg as u32);
        }
        Insn::TypeOf(slot)
        | Insn::ToNumber(slot)
        | Insn::ToNumeric(slot)
        | Insn::Neg(slot)
        | Insn::BitNot(slot)
        | Insn::Inc(slot)
        | Insn::Dec(slot)
        | Insn::CallArg0(slot)
        | Insn::GetIterator(slot)
        | Insn::CopyRestArgs(slot)
        | Insn::CallRuntimeIsTrue(slot)
        | Insn::CallRuntimeIsFalse(slot) => out.imms.push(imm_i64(*slot as i64)),
        Insn::CreateEmptyArray(slot) => out.imms.push(imm_i64(*slot as i64)),
        Insn::CallArgs2(slot, a, b)
        | Insn::CallThis1(slot, a, b)
        | Insn::StObjByValue(slot, a, b)
        | Insn::StSuperByValue(slot, a, b)
        | Insn::StOwnByValue(slot, a, b)
        | Insn::StOwnByValueWithNameSet(slot, a, b)
        | Insn::CallRuntimeDefineFieldByValue(slot, a, b)
        | Insn::Apply(slot, a, b) => {
            out.imms.push(imm_i64(*slot as i64));
            out.regs.extend([*a as u32, *b as u32]);
        }
        Insn::CallArgs3(slot, a, b, c) | Insn::CallThis2(slot, a, b, c) => {
            out.imms.push(imm_i64(*slot as i64));
            out.regs.extend([*a as u32, *b as u32, *c as u32]);
        }
        Insn::CallThis3(slot, a, b, c, d) => {
            out.imms.push(imm_i64(*slot as i64));
            out.regs
                .extend([*a as u32, *b as u32, *c as u32, *d as u32]);
        }
        Insn::CallRange(slot, argc, start)
        | Insn::CallThisRange(slot, argc, start)
        | Insn::SuperCallThisRange(slot, argc, start)
        | Insn::SuperCallArrowRange(slot, argc, start)
        | Insn::NewObjRange(slot, argc, start) => {
            out.imms
                .extend([imm_i64(*slot as i64), imm_i64(*argc as i64)]);
            out.regs.push(*start as u32);
        }
        Insn::TryLdGlobalByName(slot, id)
        | Insn::TryStGlobalByName(slot, id)
        | Insn::LdObjByName(slot, id)
        | Insn::LdSuperByName(slot, id) => {
            out.imms.push(imm_i64(*slot as i64));
            out.ids.push(string_id(strings, *id)?);
        }
        Insn::StObjByName(slot, id, reg)
        | Insn::StSuperByName(slot, id, reg)
        | Insn::DefinePropertyByName(slot, id, reg)
        | Insn::DefineFieldByName(slot, id, reg)
        | Insn::StOwnByName(slot, id, reg)
        | Insn::StOwnByNameWithNameSet(slot, id, reg) => {
            out.imms.push(imm_i64(*slot as i64));
            out.ids.push(string_id(strings, *id)?);
            out.regs.push(*reg as u32);
        }
        Insn::LdExternalModuleVar(index)
        | Insn::LdLocalModuleVar(index)
        | Insn::StModuleVar(index) => out.imms.push(imm_i64(*index as i64)),
        Insn::LdLexVar(level, slot) | Insn::StLexVar(level, slot) => {
            out.imms
                .extend([imm_i64(*level as i64), imm_i64(*slot as i64)]);
        }
        Insn::NewLexEnvWithName(count, literal)
        | Insn::CreateArrayWithBuffer(count, literal)
        | Insn::CreateObjectWithBuffer(count, literal) => {
            out.imms.push(imm_i64(*count as i64));
            out.ids.push(bytes(&literal_key(record, *literal as usize)));
        }
        Insn::NewLexEnv(count) => out.imms.push(imm_i64(*count as i64)),
        Insn::ThrowUndefinedIfHoleWithName(id) => out.ids.push(string_id(strings, *id)?),
        Insn::ThrowIfSuperNotCorrectCall(kind) => out.imms.push(imm_i64(*kind as i64)),
        Insn::LdPrivateProperty(ic, level, slot) | Insn::TestIn(ic, level, slot) => {
            out.imms.extend([
                imm_i64(*ic as i64),
                imm_i64(*level as i64),
                imm_i64(*slot as i64),
            ]);
        }
        Insn::StPrivateProperty(ic, level, slot, reg)
        | Insn::CallRuntimeDefinePrivateProperty(ic, level, slot, reg) => {
            out.imms.extend([
                imm_i64(*ic as i64),
                imm_i64(*level as i64),
                imm_i64(*slot as i64),
            ]);
            out.regs.push(*reg as u32);
        }
        Insn::CallRuntimeCreatePrivateProperty(count, literal) => {
            out.imms.push(imm_i64(*count as i64));
            out.ids.push(bytes(&literal_key(record, *literal as usize)));
        }
        Insn::DefineFuncShort(slot, id, length) | Insn::DefineMethodShort(slot, id, length) => {
            out.imms
                .extend([imm_i64(*slot as i64), imm_i64(*length as i64)]);
            out.ids.push(method_id(record, strings, *id)?);
        }
        Insn::DefineClassWithBuffer(slot, _ctor, literal, argc, reg) => {
            out.imms
                .extend([imm_i64(*slot as i64), imm_i64(*argc as i64)]);
            out.ids.push(bytes(
                &next_constructor_name(record, offset)
                    .unwrap_or_else(|| full_method_name(&record.name, "func_main_0")),
            ));
            out.ids.push(bytes(&literal_key(record, *literal as usize)));
            out.regs.push(*reg as u32);
        }
        Insn::DefineGetterSetterByValue(a, b, c, d) => {
            out.regs
                .extend([*a as u32, *b as u32, *c as u32, *d as u32]);
        }
        Insn::StOwnByIndex(slot, array, index) => {
            out.imms
                .extend([imm_i64(*slot as i64), imm_i64(*index as i64)]);
            out.regs.push(*array as u32);
        }
        Insn::StArraySpread(a, b) => out.regs.extend([*a as u32, *b as u32]),
        Insn::CreateObjectWithExcludedKeys(count, a, b) => {
            out.imms.push(imm_i64(*count as i64));
            out.regs.extend([*a as u32, *b as u32]);
        }
        Insn::GetTemplateObject(slot) => out.imms.push(imm_i64(*slot as i64)),
        Insn::JmpShort(_)
        | Insn::Jmp(_)
        | Insn::JmpWide(_)
        | Insn::JeqzShort(_)
        | Insn::Jeqz(_)
        | Insn::JeqzWide(_)
        | Insn::JnezShort(_)
        | Insn::Jnez(_)
        | Insn::JnezWide(_) => {
            if let Some(target) = jump_target(offset, insn) {
                out.ids
                    .push(bytes(label_name_for_offset(label_names, target)?));
            }
        }
    }
    Ok(out)
}

fn next_constructor_name(record: &ClassRecord, _offset: usize) -> Option<String> {
    record
        .methods
        .iter()
        .find(|method| method.kind == MethodKind::Constructor)
        .map(|method| full_method_name(&record.name, &method.name))
}

fn pandasm_opcode(insn: &Insn) -> u32 {
    if let Insn::Pandasm(raw) = insn {
        return raw.opcode;
    }
    let mut strings = StringTable::default();
    let bytes = insn.encode(&mut strings);
    pandasm_opcode_for_bytecode(&bytes).expect("dedicated instruction must exist in Panda ISA")
}

fn literal_array_to_proto(
    array: &LiteralArray,
    record_name: &str,
    module_style: bool,
) -> io::Result<crate::proto_panda::LiteralArray> {
    let mut literals = Vec::new();
    for entry in &array.entries {
        if module_style {
            literals.push(module_literal_to_proto(entry)?);
        } else {
            push_tagged_literal_to_proto(&mut literals, entry, record_name)?;
        }
    }
    Ok(crate::proto_panda::LiteralArray { literals })
}

fn module_literal_to_proto(entry: &LiteralEntry) -> io::Result<Literal> {
    match entry {
        LiteralEntry::RawU32(value) => Ok(literal_u32(LITERAL_TAG_INTEGER, *value)),
        LiteralEntry::RawU16(value) => Ok(literal_u16(LITERAL_TAG_METHOD_AFFILIATE, *value)),
        LiteralEntry::RawString(value) => Ok(literal_string(LITERAL_TAG_STRING, value)),
        other => invalid_data(format!(
            "non-module literal {other:?} in module literal array"
        )),
    }
}

fn push_tagged_literal_to_proto(
    literals: &mut Vec<Literal>,
    entry: &LiteralEntry,
    record_name: &str,
) -> io::Result<()> {
    let tag = tagged_literal_tag(entry)?;
    literals.push(literal_u8(LITERAL_TAG_TAGVALUE, tag));
    literals.push(tagged_literal_value_to_proto(entry, record_name)?);
    Ok(())
}

fn tagged_literal_tag(entry: &LiteralEntry) -> io::Result<u8> {
    Ok(match entry {
        LiteralEntry::TaggedBool(_) => LITERAL_TAG_BOOL,
        LiteralEntry::TaggedInteger(_) => LITERAL_TAG_INTEGER,
        LiteralEntry::TaggedFloat(_) => LITERAL_TAG_FLOAT,
        LiteralEntry::TaggedDouble(_) => LITERAL_TAG_DOUBLE,
        LiteralEntry::TaggedString(_) => LITERAL_TAG_STRING,
        LiteralEntry::TaggedMethod(_) | LiteralEntry::TaggedMethodName(_) => LITERAL_TAG_METHOD,
        LiteralEntry::TaggedGeneratorMethod(_) | LiteralEntry::TaggedGeneratorMethodName(_) => {
            LITERAL_TAG_GENERATOR_METHOD
        }
        LiteralEntry::TaggedAsyncGeneratorMethod(_)
        | LiteralEntry::TaggedAsyncGeneratorMethodName(_) => LITERAL_TAG_ASYNC_GENERATOR_METHOD,
        LiteralEntry::TaggedGetter(_) | LiteralEntry::TaggedGetterName(_) => LITERAL_TAG_GETTER,
        LiteralEntry::TaggedSetter(_) | LiteralEntry::TaggedSetterName(_) => LITERAL_TAG_SETTER,
        LiteralEntry::TaggedAccessor(_) => LITERAL_TAG_ACCESSOR,
        LiteralEntry::TaggedMethodAffiliate(_) => LITERAL_TAG_METHOD_AFFILIATE,
        LiteralEntry::TaggedLiteralBufferIndex(_) => LITERAL_TAG_LITERAL_BUFFER_INDEX,
        LiteralEntry::TaggedLiteralArrayIndex(_) => LITERAL_TAG_LITERAL_ARRAY,
        LiteralEntry::TaggedBuiltinTypeIndex(_) => LITERAL_TAG_BUILTIN_TYPE_INDEX,
        LiteralEntry::TaggedEtsImplements(_) => LITERAL_TAG_ETS_IMPLEMENTS,
        LiteralEntry::TaggedNull => LITERAL_TAG_NULL,
        other => {
            return invalid_data(format!(
                "module raw literal {other:?} in tagged literal array"
            ))
        }
    })
}

fn tagged_literal_value_to_proto(entry: &LiteralEntry, record_name: &str) -> io::Result<Literal> {
    Ok(match entry {
        LiteralEntry::TaggedBool(value) => literal_bool(LITERAL_TAG_BOOL, *value),
        LiteralEntry::TaggedInteger(value) => literal_u32(LITERAL_TAG_INTEGER, *value),
        LiteralEntry::TaggedFloat(value) => literal_f32(LITERAL_TAG_FLOAT, *value),
        LiteralEntry::TaggedDouble(value) => literal_f64(LITERAL_TAG_DOUBLE, *value),
        LiteralEntry::TaggedString(value) => literal_string(LITERAL_TAG_STRING, value),
        LiteralEntry::TaggedMethod(value) => literal_u32(LITERAL_TAG_METHOD, *value),
        LiteralEntry::TaggedMethodName(value) => {
            literal_string(LITERAL_TAG_METHOD, &full_method_name(record_name, value))
        }
        LiteralEntry::TaggedGeneratorMethod(value) => {
            literal_u32(LITERAL_TAG_GENERATOR_METHOD, *value)
        }
        LiteralEntry::TaggedGeneratorMethodName(value) => literal_string(
            LITERAL_TAG_GENERATOR_METHOD,
            &full_method_name(record_name, value),
        ),
        LiteralEntry::TaggedAsyncGeneratorMethod(value) => {
            literal_u32(LITERAL_TAG_ASYNC_GENERATOR_METHOD, *value)
        }
        LiteralEntry::TaggedAsyncGeneratorMethodName(value) => literal_string(
            LITERAL_TAG_ASYNC_GENERATOR_METHOD,
            &full_method_name(record_name, value),
        ),
        LiteralEntry::TaggedGetter(value) => literal_u32(LITERAL_TAG_GETTER, *value),
        LiteralEntry::TaggedGetterName(value) => {
            literal_string(LITERAL_TAG_GETTER, &full_method_name(record_name, value))
        }
        LiteralEntry::TaggedSetter(value) => literal_u32(LITERAL_TAG_SETTER, *value),
        LiteralEntry::TaggedSetterName(value) => {
            literal_string(LITERAL_TAG_SETTER, &full_method_name(record_name, value))
        }
        LiteralEntry::TaggedAccessor(value) => literal_u8(LITERAL_TAG_ACCESSOR, *value),
        LiteralEntry::TaggedMethodAffiliate(value) => {
            literal_u16(LITERAL_TAG_METHOD_AFFILIATE, *value)
        }
        LiteralEntry::TaggedLiteralBufferIndex(value) => {
            literal_u32(LITERAL_TAG_LITERAL_BUFFER_INDEX, *value)
        }
        LiteralEntry::TaggedLiteralArrayIndex(index) => literal_string(
            LITERAL_TAG_LITERAL_ARRAY,
            &literal_key_for_index(record_name, *index),
        ),
        LiteralEntry::TaggedBuiltinTypeIndex(value) => {
            literal_u8(LITERAL_TAG_BUILTIN_TYPE_INDEX, *value)
        }
        LiteralEntry::TaggedEtsImplements(value) => {
            literal_string(LITERAL_TAG_ETS_IMPLEMENTS, value)
        }
        LiteralEntry::TaggedNull => literal_u8(LITERAL_TAG_NULL, 0),
        other => {
            return invalid_data(format!(
                "module raw literal {other:?} in tagged literal array"
            ))
        }
    })
}

fn literal_key_for_index(record_name: &str, index: usize) -> String {
    literal_array_key(record_name, index)
}

fn literal_bool(tag: u8, value: bool) -> Literal {
    Literal {
        tag: tag as u32,
        value: Some(VariantValue {
            value: Some(variant_value::Value::ValueInt(u64::from(value))),
            r#type: VARIANT_BOOL,
        }),
    }
}

fn literal_u8(tag: u8, value: u8) -> Literal {
    Literal {
        tag: tag as u32,
        value: Some(VariantValue {
            value: Some(variant_value::Value::ValueInt(value as u64)),
            r#type: VARIANT_U8,
        }),
    }
}

fn literal_u16(tag: u8, value: u16) -> Literal {
    Literal {
        tag: tag as u32,
        value: Some(VariantValue {
            value: Some(variant_value::Value::ValueInt(value as u64)),
            r#type: VARIANT_U16,
        }),
    }
}

fn literal_u32(tag: u8, value: u32) -> Literal {
    Literal {
        tag: tag as u32,
        value: Some(VariantValue {
            value: Some(variant_value::Value::ValueInt(value as u64)),
            r#type: VARIANT_U32,
        }),
    }
}

fn literal_f32(tag: u8, value: f32) -> Literal {
    Literal {
        tag: tag as u32,
        value: Some(VariantValue {
            value: Some(variant_value::Value::ValueFloat(value)),
            r#type: VARIANT_F32,
        }),
    }
}

fn literal_f64(tag: u8, value: f64) -> Literal {
    Literal {
        tag: tag as u32,
        value: Some(VariantValue {
            value: Some(variant_value::Value::ValueDouble(value)),
            r#type: VARIANT_F64,
        }),
    }
}

fn literal_string(tag: u8, value: &str) -> Literal {
    Literal {
        tag: tag as u32,
        value: Some(VariantValue {
            value: Some(variant_value::Value::ValueStr(bytes(value))),
            r#type: VARIANT_STRING,
        }),
    }
}

fn string_id(strings: &StringTable, id: u16) -> io::Result<Vec<u8>> {
    strings.get(id as u32).map(bytes).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "missing string id for ProgramCache",
        )
    })
}

fn method_id(record: &ClassRecord, strings: &StringTable, id: u16) -> io::Result<Vec<u8>> {
    strings
        .get(id as u32)
        .map(|name| bytes(&full_method_name(&record.name, name)))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing method id for ProgramCache",
            )
        })
}

fn imm_i64(value: i64) -> ins::IType {
    ins::IType {
        r#type: Some(ins::i_type::Type::ValueInt(value)),
    }
}

fn imm_f64(value: f64) -> ins::IType {
    ins::IType {
        r#type: Some(ins::i_type::Type::ValueDouble(value)),
    }
}

fn literal_key(record: &ClassRecord, literal_index: usize) -> String {
    literal_array_key(&record.name, literal_index)
}

fn label_name_for_source_index(index: usize) -> String {
    format!("label_{index}")
}

fn label_name_for_inst_idx(inst_idx: usize) -> String {
    format!("label@{inst_idx}")
}

fn record_metadata(access_flags: u32) -> RecordMetadata {
    RecordMetadata {
        father: Some(item_metadata(access_flags)),
    }
}

fn function_metadata(
    access_flags: u32,
    expected_property_count: Option<u32>,
    slots_num: u32,
    function_kind_value: u32,
    concurrent_module_requests: &[u32],
) -> FunctionMetadata {
    FunctionMetadata {
        father: Some(item_metadata_with_annotations(
            access_flags,
            function_annotations(
                expected_property_count,
                slots_num,
                function_kind_value,
                concurrent_module_requests,
            ),
        )),
    }
}

fn item_metadata(access_flags: u32) -> ItemMetadata {
    item_metadata_with_annotations(access_flags, Vec::new())
}

fn item_metadata_with_annotations(
    access_flags: u32,
    annotations: Vec<AnnotationData>,
) -> ItemMetadata {
    ItemMetadata {
        father: Some(annotation_metadata(annotations)),
        access_flags,
    }
}

fn annotation_metadata(annotations: Vec<AnnotationData>) -> crate::proto_panda::AnnotationMetadata {
    crate::proto_panda::AnnotationMetadata {
        father: Some(Metadata {
            set_attributes: Vec::new(),
            attributes: Vec::new(),
        }),
        annotations,
    }
}

fn function_annotations(
    expected_property_count: Option<u32>,
    slots_num: u32,
    function_kind_value: u32,
    concurrent_module_requests: &[u32],
) -> Vec<AnnotationData> {
    let mut annotations = Vec::with_capacity(3);
    if function_kind_value == function_kind::CONCURRENT_FUNCTION {
        annotations.push(concurrent_module_requests_annotation(
            concurrent_module_requests,
        ));
    }
    if let Some(expected_property_count) = expected_property_count {
        annotations.push(u32_annotation(
            EXPECTED_PROPERTY_COUNT_RECORD_NAME,
            "ExpectedPropertyCount",
            expected_property_count,
        ));
    }
    annotations.push(u32_annotation(
        SLOT_NUMBER_RECORD_NAME,
        "SlotNumber",
        slots_num,
    ));
    annotations
}

fn concurrent_module_requests_annotation(requests: &[u32]) -> AnnotationData {
    AnnotationData {
        record_name: bytes(CONCURRENT_MODULE_REQUEST_RECORD_NAME),
        elements: requests
            .iter()
            .map(|request| AnnotationElement {
                name: bytes("ConcurrentModuleRequest"),
                value: Some(annotation_element::Value::Scalar(scalar_u64(
                    VALUE_TYPE_U32,
                    *request as u64,
                ))),
                value_type: annotation_element::ValueType::Scalar as i32,
            })
            .collect(),
    }
}

fn u32_annotation(record_name: &str, element_name: &str, value: u32) -> AnnotationData {
    AnnotationData {
        record_name: bytes(record_name),
        elements: vec![AnnotationElement {
            name: bytes(element_name),
            value: Some(annotation_element::Value::Scalar(scalar_u64(
                VALUE_TYPE_U32,
                value as u64,
            ))),
            value_type: annotation_element::ValueType::Scalar as i32,
        }],
    }
}

fn scalar_u64(value_type: u32, value: u64) -> ScalarValue {
    ScalarValue {
        father: Some(Value { r#type: value_type }),
        value: Some(scalar_value::Value::ValueU64(value)),
        r#type: SCALAR_UINT64,
    }
}

fn scalar_string(value_type: u32, value: String) -> ScalarValue {
    ScalarValue {
        father: Some(Value { r#type: value_type }),
        value: Some(scalar_value::Value::ValueStr(bytes(&value))),
        r#type: SCALAR_STRING,
    }
}

fn field_type(type_idx: u16) -> Type {
    match type_idx {
        0 => type_named("u8"),
        1 => type_named("u32"),
        _ => type_named("any"),
    }
}

fn type_named(name: &str) -> Type {
    Type {
        component_name: bytes(name),
        rank: 0,
        name: bytes(name),
        type_id: pandasm_type_id(name),
    }
}

fn pandasm_type_id(name: &str) -> u32 {
    match name {
        "void" => 0x01,
        "u1" => 0x02,
        "i8" => 0x03,
        "u8" => 0x04,
        "i16" => 0x05,
        "u16" => 0x06,
        "i32" => 0x07,
        "u32" => 0x08,
        "f32" => 0x09,
        "f64" => 0x0a,
        "i64" => 0x0b,
        "u64" => 0x0c,
        "any" => 0x0e,
        _ => 0x0d,
    }
}

fn default_source_location() -> SourceLocation {
    SourceLocation {
        begin: Some(default_source_position()),
        end: Some(default_source_position()),
    }
}

fn default_source_position() -> SourcePosition {
    SourcePosition { line: 0, column: 0 }
}

fn bytes(value: &str) -> Vec<u8> {
    value.as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::file::AbcFile;
    use crate::ir::builder::build_record_with_debug_and_package_for_target_with_mode;
    use prost::Message as _;

    #[test]
    fn program_cache_uses_pandasm_opcode_numbers_not_byte_values() {
        assert_eq!(pandasm_opcode(&Insn::LdaStr(0)), 62);
        assert_eq!(pandasm_opcode(&Insn::TryLdGlobalByName(0, 0)), 63);
        assert_eq!(pandasm_opcode(&Insn::CallRuntimeIsFalse(0)), 291);
        assert_eq!(label_ins("label_0").opcode, pandasm_invalid_opcode());
        assert_eq!(default_ins_debug().line_number, u64::MAX);
        assert_eq!(default_ins_debug().column_number, 0);
    }

    #[test]
    fn debug_proto_wire_fields_match_upstream_schema() {
        let debug = DebuginfoIns {
            line_number: 7,
            column_number: 3,
            whole_line: Vec::new(),
            bound_left: 0,
            bound_right: 0,
        };
        assert_eq!(debug.encode_to_vec(), vec![0x08, 0x07, 0x10, 0x03]);

        let local = crate::proto_panda::LocalVariable {
            name: b"x".to_vec(),
            signature: b"any".to_vec(),
            signature_type: b"any".to_vec(),
            reg: 2,
            start: 3,
            length: 4,
        };
        assert_eq!(
            local.encode_to_vec(),
            vec![
                0x0a, 0x01, b'x', 0x12, 0x03, b'a', b'n', b'y', 0x1a, 0x03, b'a', b'n', b'y', 0x20,
                0x02, 0x28, 0x03, 0x30, 0x04,
            ]
        );
    }

    #[test]
    fn tagged_literal_proto_uses_upstream_tagvalue_pairs() {
        let entries = vec![
            LiteralEntry::TaggedString("a".to_string()),
            LiteralEntry::TaggedBool(true),
            LiteralEntry::TaggedMethodAffiliate(7),
            LiteralEntry::TaggedNull,
        ];
        let array = LiteralArray {
            entries: entries.clone(),
        };

        let proto = literal_array_to_proto(&array, "record", false).unwrap();
        let tags = proto
            .literals
            .iter()
            .map(|literal| literal.tag as u8)
            .collect::<Vec<_>>();
        assert_eq!(
            tags,
            vec![
                LITERAL_TAG_TAGVALUE,
                LITERAL_TAG_STRING,
                LITERAL_TAG_TAGVALUE,
                LITERAL_TAG_BOOL,
                LITERAL_TAG_TAGVALUE,
                LITERAL_TAG_METHOD_AFFILIATE,
                LITERAL_TAG_TAGVALUE,
                LITERAL_TAG_NULL,
            ]
        );
        let tag_values = proto
            .literals
            .chunks(2)
            .map(|pair| literal_value_int(&pair[0]) as u8)
            .collect::<Vec<_>>();
        assert_eq!(
            tag_values,
            vec![
                LITERAL_TAG_STRING,
                LITERAL_TAG_BOOL,
                LITERAL_TAG_METHOD_AFFILIATE,
                LITERAL_TAG_NULL,
            ]
        );

        let mut method_refs = BTreeSet::new();
        let restored = literal_array_from_proto(
            proto,
            false,
            Some("record"),
            &BTreeMap::new(),
            &mut method_refs,
        )
        .unwrap();
        assert_eq!(restored.entries, entries);
    }

    #[test]
    fn module_literal_proto_keeps_upstream_raw_tags() {
        let entries = vec![
            LiteralEntry::RawU32(1),
            LiteralEntry::RawString("./dep".to_string()),
            LiteralEntry::RawU16(0),
        ];
        let array = LiteralArray {
            entries: entries.clone(),
        };

        let proto = literal_array_to_proto(&array, "record", true).unwrap();
        let tags = proto
            .literals
            .iter()
            .map(|literal| literal.tag as u8)
            .collect::<Vec<_>>();
        assert_eq!(
            tags,
            vec![
                LITERAL_TAG_INTEGER,
                LITERAL_TAG_STRING,
                LITERAL_TAG_METHOD_AFFILIATE,
            ]
        );
        assert!(!tags.contains(&LITERAL_TAG_TAGVALUE));

        let mut method_refs = BTreeSet::new();
        let restored =
            literal_array_from_proto(proto, true, None, &BTreeMap::new(), &mut method_refs)
                .unwrap();
        assert_eq!(restored.entries, entries);
    }

    fn literal_value_int(literal: &Literal) -> u64 {
        literal
            .value
            .as_ref()
            .and_then(|value| match &value.value {
                Some(variant_value::Value::ValueInt(value)) => Some(*value),
                _ => None,
            })
            .unwrap()
    }

    fn function_by_short_name<'a>(program: &'a Program, name: &str) -> Option<&'a Function> {
        program
            .function_table
            .iter()
            .find(|entry| {
                std::str::from_utf8(&entry.key)
                    .map(|key| {
                        let short = key.rsplit('.').next().unwrap_or(key);
                        short == name || short.ends_with(&format!("#*#{name}"))
                    })
                    .unwrap_or(false)
            })
            .and_then(|entry| entry.value.as_ref())
    }

    fn functions_by_short_suffix<'a>(program: &'a Program, name: &str) -> Vec<&'a Function> {
        program
            .function_table
            .iter()
            .filter(|entry| {
                std::str::from_utf8(&entry.key)
                    .map(|key| {
                        let short = key.rsplit('.').next().unwrap_or(key);
                        short == name || short.ends_with(&format!("#{name}"))
                    })
                    .unwrap_or(false)
            })
            .filter_map(|entry| entry.value.as_ref())
            .collect()
    }

    fn function_kind_by_short_name(program: &Program, name: &str) -> Option<u32> {
        function_by_short_name(program, name).map(|function| function.function_kind)
    }

    fn has_function_annotation(function: &Function, record_name: &str) -> bool {
        function
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.father.as_ref())
            .and_then(|metadata| metadata.father.as_ref())
            .is_some_and(|metadata| {
                metadata
                    .annotations
                    .iter()
                    .any(|annotation| annotation.record_name == record_name.as_bytes())
            })
    }

    fn function_annotation_u32_values(
        function: &Function,
        record_name: &str,
        element_name: &str,
    ) -> Vec<u32> {
        function
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.father.as_ref())
            .and_then(|metadata| metadata.father.as_ref())
            .into_iter()
            .flat_map(|metadata| metadata.annotations.iter())
            .filter(|annotation| annotation.record_name == record_name.as_bytes())
            .flat_map(|annotation| annotation.elements.iter())
            .filter(|element| element.name == element_name.as_bytes())
            .filter_map(|element| {
                let annotation_element::Value::Scalar(value) = element.value.as_ref()? else {
                    return None;
                };
                scalar_u64_value(value)
                    .ok()
                    .and_then(|value| u32::try_from(value).ok())
            })
            .collect()
    }

    #[test]
    fn proto_program_restores_generic_pandasm_instruction() {
        let opcode = pandasm_opcode_for_bytecode(&[0x6a]).unwrap();
        let ins = Ins {
            opcode,
            regs: Vec::new(),
            ids: Vec::new(),
            imms: Vec::new(),
            label: Vec::new(),
            set_label_val: false,
            ins_debug: None,
        };
        let mut strings = StringTable::default();
        let literals = BTreeMap::new();
        let mut refs = MethodDecodeRefs::default();

        let restored = generic_insn_from_proto(
            &ins,
            opcode,
            "Lentry/main;",
            &mut strings,
            &literals,
            &mut refs,
        )
        .unwrap();

        let Insn::Pandasm(raw) = restored else {
            panic!("generic PB opcode should restore as Insn::Pandasm");
        };
        assert_eq!(raw.opcode, opcode);
        assert_eq!(raw.bytes, vec![0x6a]);
    }

    #[test]
    fn proto_program_preserves_upstream_invalid_instruction_debug_line() {
        let ins = Ins {
            opcode: 0,
            regs: Vec::new(),
            ids: Vec::new(),
            imms: Vec::new(),
            label: Vec::new(),
            set_label_val: false,
            ins_debug: Some(DebuginfoIns {
                line_number: u64::MAX,
                column_number: 0,
                whole_line: Vec::new(),
                bound_left: 0,
                bound_right: 0,
            }),
        };

        assert_eq!(
            instruction_debug_from_proto(&ins).unwrap(),
            Some(InstructionDebugInfo {
                line_number: u32::MAX,
                column_number: 0,
            })
        );
        assert_eq!(
            ins_debug(Some(InstructionDebugInfo {
                line_number: u32::MAX,
                column_number: u32::MAX,
            }))
            .line_number,
            u64::MAX
        );
    }

    #[test]
    fn compiled_program_uses_upstream_function_kind_values() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            r#"
async function af() { return 1; }
function* gf() { yield 1; }
async function* ag() { yield 1; }
const ar = () => 1;
const aa = async () => 1;
function cf() { "use concurrent"; return 1; }
function sf() { "use sendable"; return 1; }
async function saf() { "use sendable"; return 1; }
af(); gf(); ag(); ar(); aa(); cf(); sf(); saf();
"#,
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            None,
            false,
            false,
            "ets",
            true,
            18,
            None,
            false,
            false,
            true,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 18).unwrap().unwrap();
        assert_eq!(
            function_kind_by_short_name(&program, "af"),
            Some(function_kind::ASYNC_FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "gf"),
            Some(function_kind::GENERATOR_FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "ag"),
            Some(function_kind::ASYNC_GENERATOR_FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "ar"),
            Some(function_kind::NC_FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "aa"),
            Some(function_kind::ASYNC_NC_FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "sf"),
            Some(function_kind::FUNCTION | function_kind::SENDABLE_FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "saf"),
            Some(function_kind::ASYNC_FUNCTION | function_kind::SENDABLE_FUNCTION)
        );

        let concurrent = function_by_short_name(&program, "cf").unwrap();
        assert_eq!(concurrent.function_kind, function_kind::CONCURRENT_FUNCTION);
        assert!(concurrent.concurrent_module_requests.is_empty());
        assert!(has_function_annotation(
            concurrent,
            CONCURRENT_MODULE_REQUEST_RECORD_NAME
        ));
    }

    #[test]
    fn concurrent_function_records_upstream_module_request_sequence() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            r#"
import { a, b } from "./dep1";
import { c } from "./dep2";
function cf() {
  "use concurrent";
  return a + a + b + c;
}
cf();
"#,
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            None,
            false,
            false,
            "ets",
            true,
            18,
            None,
            false,
            false,
            true,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 18).unwrap().unwrap();
        let concurrent = function_by_short_name(&program, "cf").unwrap();
        assert_eq!(concurrent.function_kind, function_kind::CONCURRENT_FUNCTION);
        assert_eq!(concurrent.concurrent_module_requests, vec![0, 0, 0, 1]);
        assert_eq!(
            function_annotation_u32_values(
                concurrent,
                CONCURRENT_MODULE_REQUEST_RECORD_NAME,
                "ConcurrentModuleRequest",
            ),
            vec![0, 0, 0, 1]
        );
    }

    #[test]
    fn nested_concurrent_function_uses_nearest_upstream_owner_for_module_requests() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            r#"
import { a } from "./dep1";
import { b } from "./dep2";
function outer() {
  "use concurrent";
  let local = 1;
  function inner() {
    "use concurrent";
    return b;
  }
  function nested() {
    return local + a;
  }
  return local + a;
}
outer();
"#,
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            None,
            false,
            false,
            "ets",
            true,
            18,
            None,
            false,
            false,
            true,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 18).unwrap().unwrap();
        let outer = function_by_short_name(&program, "outer").unwrap();
        let inner = function_by_short_name(&program, "inner").unwrap();
        assert_eq!(outer.function_kind, function_kind::CONCURRENT_FUNCTION);
        assert_eq!(inner.function_kind, function_kind::CONCURRENT_FUNCTION);
        assert_eq!(outer.concurrent_module_requests, vec![0, 0]);
        assert_eq!(inner.concurrent_module_requests, vec![1]);
    }

    #[test]
    fn concurrent_methods_use_upstream_function_kind_and_module_requests() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            r#"
import { a } from "./dep";
const obj = {
  method() {
    "use concurrent";
    return a;
  },
};
class C {
  method() {
    "use concurrent";
    return a;
  }
}
obj.method();
new C().method();
"#,
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            None,
            false,
            false,
            "ets",
            true,
            18,
            None,
            false,
            false,
            true,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 18).unwrap().unwrap();
        let methods = functions_by_short_suffix(&program, "method");
        assert_eq!(methods.len(), 2);
        for method in methods {
            assert_eq!(method.function_kind, function_kind::CONCURRENT_FUNCTION);
            assert_eq!(method.concurrent_module_requests, vec![0]);
        }
    }

    #[test]
    fn arrow_expression_body_use_concurrent_string_is_not_a_directive() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            r#"
const cf = () => "use concurrent";
cf();
"#,
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            None,
            false,
            false,
            "ets",
            true,
            18,
            None,
            false,
            false,
            true,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 18).unwrap().unwrap();
        assert_eq!(
            function_kind_by_short_name(&program, "cf"),
            Some(function_kind::NC_FUNCTION)
        );
    }

    #[test]
    fn concurrent_function_rejects_upstream_invalid_forms_and_captures() {
        fn compile_error(source: &str) -> String {
            let mut file = AbcFile::new();
            build_record_with_debug_and_package_for_target_with_mode(
                source,
                "@bundle:demo/entry/ets/pages/Index",
                &mut file.strings,
                &mut file.literal_arrays,
                Some("entry/src/main/ets/pages/Index.ets"),
                None,
                false,
                false,
                "ets",
                true,
                18,
                None,
                false,
                false,
                true,
            )
            .unwrap_err()
            .to_string()
        }

        let cases = [
            (
                r#"let a = 1;
function cf() {
  "use concurrent";
  return a;
}
"#,
                "Concurrent function should only use import variable or local variable, 'a' is not one of them",
            ),
            (
                r#"import * as ns from "./dep";
function cf() {
  "use concurrent";
  return ns.a;
}
"#,
                "Concurrent function should only use import variable or local variable, 'ns' is not one of them",
            ),
            (
                r#"function* cf() {
  "use concurrent";
  yield 1;
}
"#,
                "Concurrent function should only be function declaration",
            ),
            (
                r#"const cf = () => {
  "use concurrent";
  return 1;
};
"#,
                "Concurrent function should only be function declaration",
            ),
        ];

        for (source, expected) in cases {
            let error = compile_error(source);
            assert!(
                error.contains(expected),
                "expected {expected:?} in {error:?}"
            );
        }
    }

    #[test]
    fn sendable_function_kind_bit_follows_upstream_api_gate() {
        fn compile_for_target(target_api_version: u32) -> Program {
            let mut file = AbcFile::new();
            let record = build_record_with_debug_and_package_for_target_with_mode(
                r#"
function sf() { "use sendable"; return 1; }
async function saf() { "use sendable"; return 1; }
sf(); saf();
"#,
                "@bundle:demo/entry/ets/pages/Index",
                &mut file.strings,
                &mut file.literal_arrays,
                Some("entry/src/main/ets/pages/Index.ets"),
                None,
                false,
                false,
                "ets",
                true,
                target_api_version,
                None,
                false,
                false,
                true,
            )
            .unwrap();
            file.add_record(record);
            program_for_record(&file, 0, target_api_version)
                .unwrap()
                .unwrap()
        }

        let api11 = compile_for_target(11);
        assert_eq!(
            function_kind_by_short_name(&api11, "sf"),
            Some(function_kind::FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&api11, "saf"),
            Some(function_kind::ASYNC_FUNCTION)
        );

        let api18 = compile_for_target(18);
        assert_eq!(
            function_kind_by_short_name(&api18, "sf"),
            Some(function_kind::FUNCTION | function_kind::SENDABLE_FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&api18, "saf"),
            Some(function_kind::ASYNC_FUNCTION | function_kind::SENDABLE_FUNCTION)
        );
    }

    #[test]
    fn object_methods_use_upstream_method_function_kind_and_literal_tags() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            r#"
const obj = {
  m() { return 1; },
  async am() { return 1; },
  *gm() { yield 1; },
  async *agm() { yield 1; },
};
obj.m(); obj.am(); obj.gm(); obj.agm();
"#,
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            None,
            false,
            false,
            "ets",
            true,
            18,
            None,
            false,
            false,
            true,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 18).unwrap().unwrap();
        assert_eq!(
            function_kind_by_short_name(&program, "m"),
            Some(function_kind::NONE)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "gm"),
            Some(function_kind::NONE)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "am"),
            Some(function_kind::ASYNC_FUNCTION)
        );
        assert_eq!(
            function_kind_by_short_name(&program, "agm"),
            Some(function_kind::ASYNC_FUNCTION)
        );

        let entries = file
            .literal_arrays
            .iter()
            .flat_map(|array| array.entries.iter())
            .collect::<Vec<_>>();
        assert!(entries.iter().any(|entry| matches!(
            entry,
            LiteralEntry::TaggedGeneratorMethodName(name) if name.ends_with("gm")
        )));
        assert!(entries.iter().any(|entry| matches!(
            entry,
            LiteralEntry::TaggedAsyncGeneratorMethodName(name) if name.ends_with("agm")
        )));
    }

    #[test]
    fn abc2program_label_style_uses_instruction_index_labels() {
        assert_eq!(instruction_index_at_offset(&[0, 2, 5], 0), 0);
        assert_eq!(instruction_index_at_offset(&[0, 2, 5], 2), 1);
        assert_eq!(instruction_index_at_offset(&[0, 2, 5], 7), 3);
        assert_eq!(instruction_index_at_offset(&[0, 2, 5], 3), 3);

        let mut file = AbcFile::new();
        let mut record = build_record_with_debug_and_package_for_target_with_mode(
            "let x = 0; if (x) { x = 1; } else { x = 2; } x;",
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            Some("entry"),
            false,
            false,
            "ets",
            true,
            12,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        for method in &mut record.methods {
            method.label_style = MethodLabelStyle::Abc2Program;
        }
        file.add_record(record);

        let program = program_for_record(&file, 0, 12).unwrap().unwrap();
        let labels = program
            .function_table
            .iter()
            .filter_map(|entry| entry.value.as_ref())
            .flat_map(|function| function.ins.iter())
            .filter(|ins| ins.set_label_val)
            .map(|ins| ins.label.as_slice())
            .collect::<Vec<_>>();
        let jump_ids = program
            .function_table
            .iter()
            .filter_map(|entry| entry.value.as_ref())
            .flat_map(|function| function.ins.iter())
            .flat_map(|ins| ins.ids.iter())
            .filter(|id| id.starts_with(b"label@"))
            .map(Vec::as_slice)
            .collect::<Vec<_>>();

        assert!(!labels.is_empty());
        assert!(!jump_ids.is_empty());
        assert!(labels.iter().all(|label| label.starts_with(b"label@")));
        assert!(jump_ids.iter().all(|id| id.starts_with(b"label@")));
        assert!(labels.iter().all(|label| !label.starts_with(b"L_")));
        assert!(jump_ids.iter().all(|id| !id.starts_with(b"L_")));
    }

    #[test]
    fn compiled_record_can_build_proto_program() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            "let x = 1; x;",
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            None,
            false,
            false,
            "ets",
            true,
            12,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 12).unwrap().unwrap();
        assert_eq!(program.record_table.len(), 3);
        assert_eq!(
            program.record_table[0].key,
            b"@bundle:demo/entry/ets/pages/Index"
        );
        assert_eq!(
            program.record_table[1].key,
            b"_ESConcurrentModuleRequestsAnnotation"
        );
        assert_eq!(program.record_table[2].key, b"_ESSlotNumberAnnotation");
        assert!(!program
            .record_table
            .iter()
            .any(|record| record.key == EXPECTED_PROPERTY_COUNT_RECORD_NAME.as_bytes()));
        let user_record = program
            .record_table
            .iter()
            .find(|record| record.key == b"@bundle:demo/entry/ets/pages/Index")
            .unwrap();
        assert_eq!(user_record.key, b"@bundle:demo/entry/ets/pages/Index");
        assert_eq!(
            user_record.value.as_ref().unwrap().name,
            b"@bundle:demo/entry/ets/pages/Index"
        );
        assert!(program
            .function_table
            .iter()
            .any(|entry| entry.key == b"@bundle:demo/entry/ets/pages/Index.func_main_0"));
        let main = program
            .function_table
            .iter()
            .find(|entry| entry.key == b"@bundle:demo/entry/ets/pages/Index.func_main_0")
            .and_then(|entry| entry.value.as_ref())
            .unwrap();
        assert_eq!(main.name, b"@bundle:demo/entry/ets/pages/Index.func_main_0");
        assert_eq!(main.function_kind, 1);
        assert_eq!(main.return_type.as_ref().unwrap().type_id, 0x0e);
        assert_eq!(main.return_type.as_ref().unwrap().name, b"any");
        assert!(main.body_location.is_some());
        let body_location = main.body_location.as_ref().unwrap();
        assert_eq!(body_location.begin.as_ref().unwrap().line, 0);
        assert_eq!(body_location.begin.as_ref().unwrap().column, 0);
        assert_eq!(body_location.end.as_ref().unwrap().line, 0);
        assert_eq!(body_location.end.as_ref().unwrap().column, 0);
        assert!(main.params.iter().all(|param| param.metadata.is_none()));
        assert!(main
            .params
            .iter()
            .all(|param| param.r#type.as_ref().unwrap().type_id == 0x0e));
        assert_eq!(
            function_annotation_u32(main, SLOT_NUMBER_RECORD_NAME, "SlotNumber").unwrap(),
            Some(main.slots_num as u32)
        );
        assert_eq!(
            function_annotation_u32(
                main,
                EXPECTED_PROPERTY_COUNT_RECORD_NAME,
                "ExpectedPropertyCount"
            )
            .unwrap(),
            None
        );
        let user_record = user_record.value.as_ref().unwrap();
        let field_type_ids = user_record
            .field_list
            .iter()
            .map(|field| {
                field
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.field_type.as_ref())
                    .map(|field_type| (field.name.as_slice(), field_type.type_id))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(field_type_ids.contains(&(b"moduleRecordIdx".as_slice(), 0x08)));
    }

    #[test]
    fn api18_program_includes_expected_property_count_record_stub() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            "let x = 1; x;",
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            None,
            false,
            false,
            "ets",
            true,
            18,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 18).unwrap().unwrap();
        let record_keys = program
            .record_table
            .iter()
            .map(|record| record.key.as_slice())
            .collect::<Vec<_>>();
        assert_eq!(
            record_keys,
            vec![
                b"@bundle:demo/entry/ets/pages/Index".as_slice(),
                CONCURRENT_MODULE_REQUEST_RECORD_NAME.as_bytes(),
                EXPECTED_PROPERTY_COUNT_RECORD_NAME.as_bytes(),
                SLOT_NUMBER_RECORD_NAME.as_bytes(),
            ]
        );
        let main = program
            .function_table
            .iter()
            .find(|entry| entry.key == b"@bundle:demo/entry/ets/pages/Index.func_main_0")
            .and_then(|entry| entry.value.as_ref())
            .unwrap();
        assert_eq!(
            function_annotation_u32(
                main,
                EXPECTED_PROPERTY_COUNT_RECORD_NAME,
                "ExpectedPropertyCount"
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn program_for_file_uses_upstream_map_order_for_tables() {
        let mut file = AbcFile::new();
        let z_record = build_record_with_debug_and_package_for_target_with_mode(
            "globalThis.zeta = \"z\";",
            "z/entry",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("z.ts"),
            None,
            false,
            false,
            "ets",
            true,
            12,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        let a_record = build_record_with_debug_and_package_for_target_with_mode(
            "globalThis.alpha = \"a\";",
            "a/entry",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("a.ts"),
            None,
            false,
            false,
            "ets",
            true,
            12,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        file.add_record(z_record);
        file.add_record(a_record);

        let program = program_for_file(&file, 12).unwrap();
        let record_keys = program
            .record_table
            .iter()
            .map(|record| String::from_utf8(record.key.clone()).unwrap())
            .collect::<Vec<_>>();
        let mut sorted_record_keys = record_keys.clone();
        sorted_record_keys.sort();
        assert_eq!(record_keys, sorted_record_keys);

        let function_keys = program
            .function_table
            .iter()
            .map(|function| String::from_utf8(function.key.clone()).unwrap())
            .collect::<Vec<_>>();
        let mut sorted_function_keys = function_keys.clone();
        sorted_function_keys.sort();
        assert_eq!(function_keys, sorted_function_keys);

        let strings = program
            .strings
            .iter()
            .map(|value| String::from_utf8(value.clone()).unwrap())
            .collect::<Vec<_>>();
        let mut sorted_strings = strings.clone();
        sorted_strings.sort();
        sorted_strings.dedup();
        assert_eq!(strings, sorted_strings);
    }

    #[test]
    fn compiled_program_uses_generation_ordered_labels() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            "let x = 0; if (x) { x = 1; } else { x = 2; } x;",
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            Some("entry"),
            false,
            false,
            "ets",
            true,
            12,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        let expected_labels = record.methods[0]
            .program_labels
            .iter()
            .map(|label| label.name.as_bytes().to_vec())
            .collect::<Vec<_>>();
        assert!(!expected_labels.is_empty());
        file.add_record(record);

        let program = program_for_record(&file, 0, 12).unwrap().unwrap();
        let main = program
            .function_table
            .iter()
            .find(|entry| entry.key == b"@bundle:demo/entry/ets/pages/Index.func_main_0")
            .and_then(|entry| entry.value.as_ref())
            .unwrap();
        let actual_labels = main
            .ins
            .iter()
            .filter(|ins| ins.set_label_val)
            .map(|ins| ins.label.clone())
            .collect::<Vec<_>>();
        assert_eq!(actual_labels, expected_labels);
    }

    #[test]
    fn compiled_try_catch_keeps_pandasm_jump_patched() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            "try { globalThis.x = 1; } catch (e) { globalThis.x = 2; }",
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            Some("entry"),
            false,
            false,
            "ets",
            true,
            12,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        let main = record
            .methods
            .iter()
            .find(|method| method.name == "func_main_0")
            .unwrap();
        let jump_offsets = main
            .pandasm_insns
            .iter()
            .filter_map(|insn| match insn {
                Insn::JmpShort(offset) => Some(*offset as i32),
                Insn::Jmp(offset) => Some(*offset as i32),
                Insn::JmpWide(offset) => Some(*offset),
                Insn::JeqzShort(offset) => Some(*offset as i32),
                Insn::Jeqz(offset) => Some(*offset as i32),
                Insn::JeqzWide(offset) => Some(*offset),
                Insn::JnezShort(offset) => Some(*offset as i32),
                Insn::Jnez(offset) => Some(*offset as i32),
                Insn::JnezWide(offset) => Some(*offset),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!jump_offsets.is_empty());
        assert!(jump_offsets.iter().all(|offset| *offset != 0));
    }

    #[test]
    fn proto_program_can_restore_record_methods_literals_and_jumps() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            "let x = 0; if (x) { x = 1; } else { x = 2; } x;",
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            Some("entry"),
            false,
            false,
            "ets",
            true,
            12,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 12).unwrap().unwrap();
        let function_count = program.function_table.len();
        let literal_count = program.literal_array_table.len();
        assert!(program.function_table.iter().all(|entry| entry
            .key
            .starts_with(b"@bundle:demo/entry/ets/pages/Index.")));
        assert!(program
            .function_table
            .iter()
            .filter_map(|entry| entry.value.as_ref())
            .flat_map(|function| function.ins.iter())
            .any(|ins| ins.ins_debug.is_some()));
        let mut restored = AbcFile::new();

        let appended = append_program_to_file(&mut restored, program).unwrap();

        assert_eq!(appended, vec![0]);
        assert_eq!(restored.records.len(), 1);
        assert_eq!(restored.records[0].name, file.records[0].name);
        assert_eq!(restored.records[0].methods.len(), function_count);
        assert_eq!(restored.literal_arrays.len(), literal_count);
        assert!(restored.records[0]
            .methods
            .iter()
            .any(|method| method.name == "func_main_0"
                && !method.bytecode.is_empty()
                && !method.pandasm_insns.is_empty()));
        let restored_main = restored.records[0]
            .methods
            .iter()
            .find(|method| method.name == "func_main_0")
            .unwrap();
        let original_main = file.records[0]
            .methods
            .iter()
            .find(|method| method.name == "func_main_0")
            .unwrap();
        assert_eq!(restored_main.bytecode, original_main.bytecode);
        assert_eq!(restored_main.line_ops, original_main.line_ops);
        assert_eq!(restored_main.pandasm_debug, original_main.pandasm_debug);
        assert!(program_for_record(&restored, 0, 12).unwrap().is_some());
    }

    #[test]
    fn proto_program_restores_debug_line_ops_for_straight_line_code() {
        let mut file = AbcFile::new();
        let record = build_record_with_debug_and_package_for_target_with_mode(
            "let x = 1; x;",
            "@bundle:demo/entry/ets/pages/Index",
            &mut file.strings,
            &mut file.literal_arrays,
            Some("entry/src/main/ets/pages/Index.ets"),
            Some("entry"),
            false,
            false,
            "ets",
            true,
            12,
            None,
            false,
            false,
            false,
        )
        .unwrap();
        file.add_record(record);

        let program = program_for_record(&file, 0, 12).unwrap().unwrap();
        let mut restored = AbcFile::new();
        append_program_to_file(&mut restored, program).unwrap();

        let original_main = file.records[0]
            .methods
            .iter()
            .find(|method| method.name == "func_main_0")
            .unwrap();
        let restored_main = restored.records[0]
            .methods
            .iter()
            .find(|method| method.name == "func_main_0")
            .unwrap();
        assert_eq!(restored_main.pandasm_debug, original_main.pandasm_debug);
        assert_eq!(restored_main.line_ops, original_main.line_ops);
    }

    #[test]
    fn program_cache_assigns_inline_cache_slots_before_serializing() {
        let mut insns = vec![
            Insn::TryLdGlobalByName(0, 0),
            Insn::CallArg0(0),
            Insn::CreateObjectWithBuffer(0, 0),
        ];

        let slots = assign_inline_cache_slots(&mut insns);

        assert_eq!(slots, 4);
        assert!(matches!(insns[0], Insn::TryLdGlobalByName(0, 0)));
        assert!(matches!(insns[1], Insn::CallArg0(1)));
        assert!(matches!(insns[2], Insn::CreateObjectWithBuffer(3, 0)));
    }
}
