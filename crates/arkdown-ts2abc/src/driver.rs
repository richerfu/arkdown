use std::path::{Path, PathBuf};

use arkdown_abc::{AbcCompileOptions, AbcCompiler, AbcModule};

use crate::error::Ts2abcError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ts2abcInput {
    pub path: PathBuf,
    pub record_name: String,
    pub source_file: Option<String>,
    pub source_map: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ts2abcOptions {
    pub inputs: Vec<Ts2abcInput>,
    pub output: PathBuf,
    pub target_api_version: u32,
    pub target_api_sub_version: Option<String>,
    pub debug_info: bool,
    pub opt_level: u8,
    pub merge_abc: bool,
    pub module: bool,
    pub commonjs: bool,
    pub cache_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ts2abcOutput {
    pub abc_path: PathBuf,
    pub compiled_records: Vec<String>,
    pub target_api_version: u32,
    pub debug_info: bool,
}

pub trait BytecodeBackend {
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ArkdownAbcBackend;

impl BytecodeBackend for ArkdownAbcBackend {
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
        if options.inputs.is_empty() {
            return Err(Ts2abcError::MissingInput);
        }

        let mut modules = Vec::with_capacity(options.inputs.len());
        for input in &options.inputs {
            if !input.path.exists() {
                return Err(Ts2abcError::MissingSource(input.path.clone()));
            }
            let source = fs_err::read_to_string(&input.path)?;
            let source_map = input
                .source_map
                .as_deref()
                .map(|source_map| normalize_source_map(source_map, input.source_file.as_deref()))
                .or_else(|| {
                    input.source_file.as_ref().map(|source_file| {
                        format!(
                            r#"{{"version":3,"sources":["{}"],"mappings":""}}"#,
                            json_escape(source_file)
                        )
                    })
                });
            modules.push(AbcModule {
                record_name: input.record_name.clone(),
                source,
                source_map,
            });
        }

        let compiler = AbcCompiler::new(AbcCompileOptions {
            debug_info: options.debug_info,
            opt_level: options.opt_level,
            target_api_version: options.target_api_version,
            merge_abc: options.merge_abc,
            module: options.module,
            cache_path: options.cache_file.clone(),
        });
        let artifact = compiler.compile(modules, &options.output)?;
        Ok(Ts2abcOutput {
            abc_path: artifact.path,
            compiled_records: options
                .inputs
                .iter()
                .map(|input| input.record_name.clone())
                .collect(),
            target_api_version: artifact.api_version,
            debug_info: artifact.has_debug_info,
        })
    }
}

pub fn compile_to_abc(options: Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
    ArkdownAbcBackend.compile(&options)
}

pub(crate) fn default_record_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("record")
        .to_string()
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}

fn normalize_source_map(source_map: &str, source_file: Option<&str>) -> String {
    let Some(source_file) = source_file else {
        return source_map.to_string();
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(source_map) else {
        return source_map.to_string();
    };
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "sources".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String(source_file.to_string())]),
        );
    }
    value.to_string()
}
