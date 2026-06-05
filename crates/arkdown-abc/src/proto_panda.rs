//! Rust protobuf model for upstream `protoPanda` ProgramCache files.
//!
//! The message names and field numbers mirror
//! `arkcompiler_ets_frontend/merge_abc/protos/*.proto`.  `arkdown-abc` keeps
//! these definitions in source instead of running `prost-build` at compile time
//! so the workspace does not depend on a system `protoc` binary.

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Value {
    #[prost(uint32, tag = "1")]
    pub r#type: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Type {
    #[prost(bytes = "vec", tag = "1")]
    pub component_name: Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub rank: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub name: Vec<u8>,
    #[prost(uint32, tag = "4")]
    pub type_id: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScalarValue {
    #[prost(message, optional, tag = "1")]
    pub father: Option<Value>,
    #[prost(oneof = "scalar_value::Value", tags = "2, 3, 4, 5, 6, 7")]
    pub value: Option<scalar_value::Value>,
    #[prost(enumeration = "scalar_value::VariantValueType", tag = "8")]
    pub r#type: i32,
}

pub mod scalar_value {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum VariantValueType {
        Uint64 = 0,
        Float = 1,
        Double = 2,
        String = 3,
        PandasmType = 4,
        AnnotationData = 5,
    }

    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Value {
        #[prost(uint64, tag = "2")]
        ValueU64(u64),
        #[prost(float, tag = "3")]
        ValueFloat(f32),
        #[prost(double, tag = "4")]
        ValueDouble(f64),
        #[prost(bytes, tag = "5")]
        ValueStr(Vec<u8>),
        #[prost(message, tag = "6")]
        ValueType(super::Type),
        #[prost(message, tag = "7")]
        ValueAnno(super::AnnotationData),
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArrayValue {
    #[prost(message, optional, tag = "1")]
    pub father: Option<Value>,
    #[prost(uint32, tag = "2")]
    pub component_type: u32,
    #[prost(message, repeated, tag = "3")]
    pub values: Vec<ScalarValue>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnnotationElement {
    #[prost(bytes = "vec", tag = "1")]
    pub name: Vec<u8>,
    #[prost(oneof = "annotation_element::Value", tags = "2, 3")]
    pub value: Option<annotation_element::Value>,
    #[prost(enumeration = "annotation_element::ValueType", tag = "4")]
    pub value_type: i32,
}

pub mod annotation_element {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum ValueType {
        Scalar = 0,
        Array = 1,
    }

    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Value {
        #[prost(message, tag = "2")]
        Scalar(super::ScalarValue),
        #[prost(message, tag = "3")]
        Array(super::ArrayValue),
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnnotationData {
    #[prost(bytes = "vec", tag = "1")]
    pub record_name: Vec<u8>,
    #[prost(message, repeated, tag = "2")]
    pub elements: Vec<AnnotationElement>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Metadata {
    #[prost(bytes = "vec", repeated, tag = "1")]
    pub set_attributes: Vec<Vec<u8>>,
    #[prost(message, repeated, tag = "2")]
    pub attributes: Vec<metadata::Attributes>,
}

pub mod metadata {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Attributes {
        #[prost(bytes = "vec", tag = "1")]
        pub key: Vec<u8>,
        #[prost(bytes = "vec", repeated, tag = "2")]
        pub value: Vec<Vec<u8>>,
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnnotationMetadata {
    #[prost(message, optional, tag = "1")]
    pub father: Option<Metadata>,
    #[prost(message, repeated, tag = "2")]
    pub annotations: Vec<AnnotationData>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ItemMetadata {
    #[prost(message, optional, tag = "1")]
    pub father: Option<AnnotationMetadata>,
    #[prost(uint32, tag = "2")]
    pub access_flags: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RecordMetadata {
    #[prost(message, optional, tag = "1")]
    pub father: Option<ItemMetadata>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FieldMetadata {
    #[prost(message, optional, tag = "1")]
    pub father: Option<ItemMetadata>,
    #[prost(message, optional, tag = "2")]
    pub field_type: Option<Type>,
    #[prost(message, optional, tag = "3")]
    pub value: Option<ScalarValue>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FunctionMetadata {
    #[prost(message, optional, tag = "1")]
    pub father: Option<ItemMetadata>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ParamMetadata {
    #[prost(message, optional, tag = "1")]
    pub father: Option<AnnotationMetadata>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileLocation {
    #[prost(bytes = "vec", tag = "1")]
    pub whole_line: Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub bound_left: u64,
    #[prost(uint64, tag = "3")]
    pub bound_right: u64,
    #[prost(uint64, tag = "4")]
    pub line_number: u64,
    #[prost(bool, tag = "5")]
    pub is_defined: bool,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SourcePosition {
    #[prost(uint64, tag = "1")]
    pub line: u64,
    #[prost(uint64, tag = "2")]
    pub column: u64,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SourceLocation {
    #[prost(message, optional, tag = "1")]
    pub begin: Option<SourcePosition>,
    #[prost(message, optional, tag = "2")]
    pub end: Option<SourcePosition>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DebuginfoIns {
    #[prost(uint32, tag = "1")]
    pub line_number: u32,
    #[prost(uint32, tag = "2")]
    pub column_number: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LocalVariable {
    #[prost(bytes = "vec", tag = "1")]
    pub name: Vec<u8>,
    #[prost(int32, tag = "2")]
    pub reg: i32,
    #[prost(uint32, tag = "3")]
    pub start: u32,
    #[prost(uint32, tag = "4")]
    pub length: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Label {
    #[prost(bytes = "vec", tag = "1")]
    pub name: Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub file_location: Option<FileLocation>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Ins {
    #[prost(uint32, tag = "1")]
    pub opcode: u32,
    #[prost(uint32, repeated, tag = "2")]
    pub regs: Vec<u32>,
    #[prost(bytes = "vec", repeated, tag = "3")]
    pub ids: Vec<Vec<u8>>,
    #[prost(message, repeated, tag = "4")]
    pub imms: Vec<ins::IType>,
    #[prost(bytes = "vec", tag = "5")]
    pub label: Vec<u8>,
    #[prost(bool, tag = "6")]
    pub set_label_val: bool,
    #[prost(message, optional, tag = "7")]
    pub ins_debug: Option<DebuginfoIns>,
}

pub mod ins {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct IType {
        #[prost(oneof = "i_type::Type", tags = "1, 2")]
        pub r#type: Option<i_type::Type>,
    }

    pub mod i_type {
        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum Type {
            #[prost(int64, tag = "1")]
            ValueInt(i64),
            #[prost(double, tag = "2")]
            ValueDouble(f64),
        }
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Field {
    #[prost(message, optional, tag = "1")]
    pub r#type: Option<Type>,
    #[prost(bytes = "vec", tag = "2")]
    pub name: Vec<u8>,
    #[prost(message, optional, tag = "3")]
    pub metadata: Option<FieldMetadata>,
    #[prost(uint64, tag = "4")]
    pub line_of_def: u64,
    #[prost(bytes = "vec", tag = "5")]
    pub whole_line: Vec<u8>,
    #[prost(uint64, tag = "6")]
    pub bound_left: u64,
    #[prost(uint64, tag = "7")]
    pub bound_right: u64,
    #[prost(bool, tag = "8")]
    pub is_defined: bool,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Record {
    #[prost(bytes = "vec", tag = "1")]
    pub name: Vec<u8>,
    #[prost(bool, tag = "2")]
    pub conflict: bool,
    #[prost(uint32, tag = "3")]
    pub language: u32,
    #[prost(message, optional, tag = "4")]
    pub metadata: Option<RecordMetadata>,
    #[prost(message, repeated, tag = "5")]
    pub field_list: Vec<Field>,
    #[prost(uint64, tag = "6")]
    pub params_num: u64,
    #[prost(bool, tag = "7")]
    pub body_presence: bool,
    #[prost(message, optional, tag = "8")]
    pub body_location: Option<SourceLocation>,
    #[prost(bytes = "vec", tag = "9")]
    pub source_file: Vec<u8>,
    #[prost(message, optional, tag = "10")]
    pub file_location: Option<FileLocation>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CatchBlock {
    #[prost(bytes = "vec", tag = "1")]
    pub whole_line: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub exception_record: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub try_begin_label: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub try_end_label: Vec<u8>,
    #[prost(bytes = "vec", tag = "5")]
    pub catch_begin_label: Vec<u8>,
    #[prost(bytes = "vec", tag = "6")]
    pub catch_end_label: Vec<u8>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TryCatchInfo {
    #[prost(message, repeated, tag = "1")]
    pub try_catch_labels: Vec<try_catch_info::TryCatchLabel>,
    #[prost(message, repeated, tag = "2")]
    pub try_catch_map: Vec<try_catch_info::TryCatchMap>,
    #[prost(bytes = "vec", repeated, tag = "3")]
    pub try_catch_order: Vec<Vec<u8>>,
}

pub mod try_catch_info {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TryCatchLabel {
        #[prost(bytes = "vec", tag = "1")]
        pub key: Vec<u8>,
        #[prost(uint64, tag = "2")]
        pub value: u64,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TryCatchMap {
        #[prost(bytes = "vec", tag = "1")]
        pub key: Vec<u8>,
        #[prost(message, repeated, tag = "2")]
        pub catch_blocks: Vec<super::CatchBlock>,
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Parameter {
    #[prost(message, optional, tag = "1")]
    pub r#type: Option<Type>,
    #[prost(message, optional, tag = "2")]
    pub metadata: Option<ParamMetadata>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Function {
    #[prost(bytes = "vec", tag = "1")]
    pub name: Vec<u8>,
    #[prost(uint32, tag = "2")]
    pub language: u32,
    #[prost(message, optional, tag = "3")]
    pub metadata: Option<FunctionMetadata>,
    #[prost(message, repeated, tag = "4")]
    pub label_table: Vec<function::LabelTable>,
    #[prost(message, repeated, tag = "5")]
    pub ins: Vec<Ins>,
    #[prost(message, repeated, tag = "6")]
    pub local_variable_debug: Vec<LocalVariable>,
    #[prost(bytes = "vec", tag = "7")]
    pub source_file: Vec<u8>,
    #[prost(bytes = "vec", tag = "8")]
    pub source_code: Vec<u8>,
    #[prost(message, repeated, tag = "9")]
    pub catch_blocks: Vec<CatchBlock>,
    #[prost(int64, tag = "10")]
    pub value_of_first_param: i64,
    #[prost(uint64, tag = "11")]
    pub regs_num: u64,
    #[prost(message, repeated, tag = "12")]
    pub params: Vec<Parameter>,
    #[prost(bool, tag = "13")]
    pub body_presence: bool,
    #[prost(message, optional, tag = "14")]
    pub return_type: Option<Type>,
    #[prost(message, optional, tag = "15")]
    pub body_location: Option<SourceLocation>,
    #[prost(message, optional, tag = "16")]
    pub file_location: Option<FileLocation>,
    #[prost(uint32, tag = "17")]
    pub function_kind: u32,
    #[prost(uint64, tag = "18")]
    pub slots_num: u64,
    #[prost(uint32, repeated, tag = "19")]
    pub concurrent_module_requests: Vec<u32>,
}

pub mod function {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct LabelTable {
        #[prost(bytes = "vec", tag = "1")]
        pub key: Vec<u8>,
        #[prost(message, optional, tag = "2")]
        pub value: Option<super::Label>,
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VariantValue {
    #[prost(oneof = "variant_value::Value", tags = "1, 2, 3, 4")]
    pub value: Option<variant_value::Value>,
    #[prost(enumeration = "variant_value::VariantValueType", tag = "5")]
    pub r#type: i32,
}

pub mod variant_value {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum VariantValueType {
        Bool = 0,
        U8 = 1,
        U16 = 2,
        U32 = 3,
        U64 = 4,
        F32 = 5,
        F64 = 6,
        String = 7,
    }

    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Value {
        #[prost(uint64, tag = "1")]
        ValueInt(u64),
        #[prost(float, tag = "2")]
        ValueFloat(f32),
        #[prost(double, tag = "3")]
        ValueDouble(f64),
        #[prost(bytes, tag = "4")]
        ValueStr(Vec<u8>),
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Literal {
    #[prost(uint32, tag = "1")]
    pub tag: u32,
    #[prost(message, optional, tag = "2")]
    pub value: Option<VariantValue>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LiteralArray {
    #[prost(message, repeated, tag = "1")]
    pub literals: Vec<Literal>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Program {
    #[prost(uint32, tag = "1")]
    pub lang: u32,
    #[prost(message, repeated, tag = "2")]
    pub record_table: Vec<program::RecordTable>,
    #[prost(message, repeated, tag = "3")]
    pub function_table: Vec<program::FunctionTable>,
    #[prost(message, repeated, tag = "4")]
    pub function_synonyms: Vec<program::FunctionSynonyms>,
    #[prost(message, repeated, tag = "5")]
    pub literal_array_table: Vec<program::LiteralArrayTable>,
    #[prost(bytes = "vec", repeated, tag = "6")]
    pub strings: Vec<Vec<u8>>,
    #[prost(message, repeated, tag = "7")]
    pub array_types: Vec<Type>,
}

pub mod program {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RecordTable {
        #[prost(bytes = "vec", tag = "1")]
        pub key: Vec<u8>,
        #[prost(message, optional, tag = "2")]
        pub value: Option<super::Record>,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct FunctionTable {
        #[prost(bytes = "vec", tag = "1")]
        pub key: Vec<u8>,
        #[prost(message, optional, tag = "2")]
        pub value: Option<super::Function>,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct FunctionSynonyms {
        #[prost(bytes = "vec", tag = "1")]
        pub key: Vec<u8>,
        #[prost(bytes = "vec", repeated, tag = "2")]
        pub value: Vec<Vec<u8>>,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct LiteralArrayTable {
        #[prost(bytes = "vec", tag = "1")]
        pub key: Vec<u8>,
        #[prost(message, optional, tag = "2")]
        pub value: Option<super::LiteralArray>,
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProgramCache {
    #[prost(uint32, tag = "1")]
    pub hash_code: u32,
    #[prost(message, optional, tag = "2")]
    pub program: Option<Program>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AbcProtoProgramsCache {
    #[prost(uint32, tag = "1")]
    pub hash_code: u32,
    #[prost(message, repeated, tag = "2")]
    pub abc_proto_programs_map: Vec<abc_proto_programs_cache::AbcProtoProgramCache>,
}

pub mod abc_proto_programs_cache {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct AbcProtoProgramCache {
        #[prost(string, tag = "1")]
        pub name: String,
        #[prost(message, optional, tag = "2")]
        pub program: Option<super::Program>,
    }
}
