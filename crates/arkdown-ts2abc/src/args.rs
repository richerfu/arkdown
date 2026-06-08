use std::path::{Path, PathBuf};

use arkdown_abc::cache::CacheFileMap;

use crate::compile_context::CompileContextInfo;
use crate::driver::{default_record_name, Ts2abcInput, Ts2abcOptions};
use crate::error::Ts2abcError;
use crate::version::{BytecodeVersion, DEFAULT_TARGET_API_VERSION};

const DEFAULT_INPUT_EXTENSION: &str = "js";
const VALID_INPUT_EXTENSIONS: &[&str] = &["js", "ts", "as", "abc"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionQuery {
    pub target_api_version: u32,
    pub target_api_sub_version: Option<String>,
    pub bytecode_version: BytecodeVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ts2abcCommand {
    PrintBcVersion(BytecodeVersion),
    PrintTargetBcVersion(VersionQuery),
    Compile(Ts2abcOptions),
}

impl Ts2abcCommand {
    /// Parse ts2abc command-line arguments.
    pub fn parse<I, S>(args: I) -> Result<Self, Ts2abcError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut parser = ArgParser::new(args.into_iter().map(Into::into).collect());
        parser.parse()
    }
}

struct ArgParser {
    args: Vec<String>,
    index: usize,
    target_api_version: u32,
    target_api_sub_version: Option<String>,
    enable_annotations: bool,
    use_define_semantic: bool,
    output: Option<PathBuf>,
    default_output_source: Option<PathBuf>,
    record_name: Option<String>,
    source_file: Option<String>,
    input_extension: String,
    module_record_field_name: Option<String>,
    cache_file: Option<PathBuf>,
    output_proto: Option<PathBuf>,
    compile_context_info: CompileContextInfo,
    enable_abc_input: bool,
    dump_deps_info: bool,
    remove_redundant_file: bool,
    npm_module_entry_list: Option<PathBuf>,
    debug_info: bool,
    merge_abc: bool,
    module: bool,
    commonjs: bool,
    opt_level: u8,
    optimize_try_catch_func: bool,
    print_bc_version: bool,
    print_bc_min_version: bool,
    print_target_bc_version: bool,
    inputs: Vec<InputSpec>,
}

enum InputSpec {
    Path(PathBuf),
    FileInfo(Ts2abcInput),
}

impl ArgParser {
    fn new(args: Vec<String>) -> Self {
        Self {
            args,
            index: 0,
            target_api_version: DEFAULT_TARGET_API_VERSION,
            target_api_sub_version: None,
            enable_annotations: false,
            use_define_semantic: false,
            output: None,
            default_output_source: None,
            record_name: None,
            source_file: None,
            input_extension: DEFAULT_INPUT_EXTENSION.to_string(),
            module_record_field_name: None,
            cache_file: None,
            output_proto: None,
            compile_context_info: CompileContextInfo::default(),
            enable_abc_input: false,
            dump_deps_info: false,
            remove_redundant_file: false,
            npm_module_entry_list: None,
            debug_info: false,
            merge_abc: false,
            module: false,
            commonjs: false,
            opt_level: 2,
            optimize_try_catch_func: true,
            print_bc_version: false,
            print_bc_min_version: false,
            print_target_bc_version: false,
            inputs: Vec::new(),
        }
    }

    fn parse(&mut self) -> Result<Ts2abcCommand, Ts2abcError> {
        while self.index < self.args.len() {
            let arg = self.args[self.index].clone();
            self.index += 1;
            self.parse_one(arg)?;
        }

        if self.print_target_bc_version {
            let version = BytecodeVersion::for_ts2abc_script(
                self.target_api_version,
                self.target_api_sub_version.as_deref(),
            );
            return Ok(Ts2abcCommand::PrintTargetBcVersion(VersionQuery {
                target_api_version: self.target_api_version,
                target_api_sub_version: self.target_api_sub_version.clone(),
                bytecode_version: version,
            }));
        }

        if self.print_bc_version {
            return Ok(Ts2abcCommand::PrintBcVersion(BytecodeVersion::new(
                13, 0, 1, 0,
            )));
        }
        if self.print_bc_min_version {
            return Ok(Ts2abcCommand::PrintBcVersion(BytecodeVersion::new(
                12, 0, 2, 0,
            )));
        }

        if self.inputs.is_empty() {
            return Err(Ts2abcError::MissingInput);
        }

        let output = self.compiler_output();
        let single_record_name = self.record_name.clone().or_else(|| {
            self.merge_abc
                .then(|| default_record_name(output.as_path()))
        });
        let merge_abc = self.merge_abc;
        let mut inputs = self
            .inputs
            .iter()
            .map(|input| match input {
                InputSpec::Path(path) => Ts2abcInput {
                    path: path.clone(),
                    output: None,
                    cache_file: None,
                    record_name: if merge_abc {
                        single_record_name
                            .clone()
                            .unwrap_or_else(|| default_record_name(path))
                    } else {
                        String::new()
                    },
                    source_file: self
                        .source_file
                        .clone()
                        .or_else(|| (!merge_abc).then(|| path.to_string_lossy().into_owned())),
                    package_name: None,
                    is_commonjs: self.commonjs,
                    is_shared_module: false,
                    origin_source_lang: String::new(),
                    source_map: None,
                },
                InputSpec::FileInfo(input) => input.clone(),
            })
            .collect::<Vec<_>>();
        self.apply_cache_file_mapping(&mut inputs)?;

        Ok(Ts2abcCommand::Compile(Ts2abcOptions {
            inputs,
            output,
            target_api_version: self.target_api_version,
            target_api_sub_version: self.target_api_sub_version.clone(),
            enable_annotations: self.enable_annotations,
            use_define_semantic: self.use_define_semantic,
            debug_info: self.debug_info,
            opt_level: self.opt_level,
            optimize_try_catch_func: self.optimize_try_catch_func,
            merge_abc: self.merge_abc,
            module: self.module,
            commonjs: self.commonjs,
            input_extension: self.input_extension.clone(),
            module_record_field_name: self.module_record_field_name.clone(),
            cache_file: self.cache_file.clone(),
            output_proto: self.output_proto.clone(),
            compile_context_info: self.compile_context_info.clone(),
            enable_abc_input: self.enable_abc_input,
            dump_deps_info: self.dump_deps_info,
            remove_redundant_file: self.remove_redundant_file,
            npm_module_entry_list: self.npm_module_entry_list.clone(),
        }))
    }

    fn parse_one(&mut self, arg: String) -> Result<(), Ts2abcError> {
        if arg == "--" {
            while self.index < self.args.len() {
                let path = PathBuf::from(&self.args[self.index]);
                if self.default_output_source.is_none() {
                    self.default_output_source = Some(path.clone());
                }
                self.inputs.push(InputSpec::Path(path));
                self.index += 1;
            }
            return Ok(());
        }
        if let Some(list_path) = arg.strip_prefix('@') {
            if self.default_output_source.is_none() {
                self.default_output_source = Some(PathBuf::from(list_path));
            }
            // @filelist: upstream options.cpp reads each line as a source input.
            // hvigor uses this for large module lists to avoid OS arg-length limits.
            let content = std::fs::read_to_string(list_path).map_err(|e| {
                Ts2abcError::Io(std::io::Error::new(
                    e.kind(),
                    format!("@filelist {list_path:?}: {e}"),
                ))
            })?;
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    self.push_filelist_line(trimmed)?;
                }
            }
            return Ok(());
        }
        if !arg.starts_with("--") {
            let path = PathBuf::from(arg);
            if self.default_output_source.is_none() {
                self.default_output_source = Some(path.clone());
            }
            self.inputs.push(InputSpec::Path(path));
            return Ok(());
        }

        let (flag, value) = split_flag_value(&arg);
        match flag {
            "--target-bc-version" => self.print_target_bc_version = true,
            "--bc-version" => self.print_bc_version = true,
            "--bc-min-version" => self.print_bc_min_version = true,
            "--target-api-version" => {
                let value = self.value(flag, value)?;
                self.target_api_version = value.parse().map_err(|_| Ts2abcError::InvalidValue {
                    flag: flag.to_string(),
                    value,
                })?;
            }
            "--target-api-sub-version" => {
                self.target_api_sub_version = Some(self.value(flag, value)?);
            }
            "--output" => self.output = Some(PathBuf::from(self.value(flag, value)?)),
            "--record-name" => self.record_name = Some(self.value(flag, value)?),
            "--source-file" => self.source_file = Some(self.value(flag, value)?),
            "--cache-file" => self.cache_file = Some(PathBuf::from(self.value(flag, value)?)),
            "--npm-module-entry-list" => {
                self.npm_module_entry_list = Some(PathBuf::from(self.value(flag, value)?));
            }
            "--debug-info" => self.debug_info = true,
            "--merge-abc" => self.merge_abc = true,
            "--module" => {
                self.module = true;
                // Upstream rejects --module + --commonjs together.
                if self.commonjs {
                    return Err(Ts2abcError::ConflictingFlags);
                }
            }
            "--commonjs" => {
                self.commonjs = true;
                if self.module {
                    return Err(Ts2abcError::ConflictingFlags);
                }
            }
            "--opt-level" => {
                let value = self.value(flag, value)?;
                self.opt_level = value.parse().map_err(|_| Ts2abcError::InvalidValue {
                    flag: flag.to_string(),
                    value,
                })?;
                if self.opt_level > 2 {
                    return Err(Ts2abcError::InvalidValue {
                        flag: flag.to_string(),
                        value: self.opt_level.to_string(),
                    });
                }
            }
            "--opt-try-catch-func" => {
                self.optimize_try_catch_func = parse_bool_flag(flag, value, true)?;
            }
            "--file-threads"
            | "--function-threads"
            | "--abc-class-threads"
            | "--perf-file"
            | "--perf-level" => {
                let _ = self.value(flag, value)?;
            }
            "--extension" => {
                let extension = self.value(flag, value)?;
                if !VALID_INPUT_EXTENSIONS.contains(&extension.as_str()) {
                    return Err(Ts2abcError::InvalidValue {
                        flag: flag.to_string(),
                        value: extension,
                    });
                }
                self.input_extension = extension;
            }
            // Upstream aot/main.cpp::GenerateProgram(): with exactly one
            // program and no global optimization, --outputProto writes a
            // protoPanda::Program snapshot and returns before ABC emit.
            "--output-proto" | "--outputProto" => {
                self.output_proto = Some(PathBuf::from(self.value(flag, value)?))
            }
            "--compile-context-info" => {
                let path = PathBuf::from(self.value(flag, value)?);
                self.compile_context_info = CompileContextInfo::read_upstream_lenient(&path)?;
            }
            "--enable-abc-input" => self.enable_abc_input = true,
            "--enable-annotations" => self.enable_annotations = true,
            "--use-define-semantic" => self.use_define_semantic = true,
            "--dump-deps-info" => self.dump_deps_info = true,
            "--remove-redundant-file" => self.remove_redundant_file = true,
            "--module-record-field-name" => {
                self.module_record_field_name = Some(self.value(flag, value)?);
            }
            // No-op flags: accepted by upstream, accepted here without action.
            "--enable-release-column" => {}
            _ => return Err(Ts2abcError::UnsupportedFlag(flag.to_string())),
        }
        Ok(())
    }

    fn value(&mut self, flag: &str, inline: Option<String>) -> Result<String, Ts2abcError> {
        if let Some(value) = inline {
            return Ok(value);
        }
        if self.index >= self.args.len() {
            return Err(Ts2abcError::MissingValue(flag.to_string()));
        }
        let value = self.args[self.index].clone();
        self.index += 1;
        Ok(value)
    }

    fn push_filelist_line(&mut self, line: &str) -> Result<(), Ts2abcError> {
        if !line.contains(';') {
            self.inputs.push(InputSpec::Path(PathBuf::from(line)));
            return Ok(());
        }

        let items = split_file_info_items(line);
        if items.is_empty() || items.len() > 7 {
            return Err(Ts2abcError::InvalidValue {
                flag: "@filelist".to_string(),
                value: line.to_string(),
            });
        }
        let Some(path) = items.first() else {
            return Err(Ts2abcError::InvalidValue {
                flag: "@filelist".to_string(),
                value: line.to_string(),
            });
        };
        let script_kind = items.get(2).map(String::as_str).unwrap_or("module");
        let is_commonjs = script_kind == "commonjs";
        let source_file = items
            .get(3)
            .cloned()
            .or_else(|| (!self.merge_abc).then(|| path.clone()));
        let package_or_output = items.get(4).cloned();
        if !self.merge_abc && package_or_output.is_none() {
            return Err(Ts2abcError::MissingValue(
                "@filelist outputfile".to_string(),
            ));
        }
        let input = Ts2abcInput {
            path: PathBuf::from(path),
            output: if self.merge_abc {
                None
            } else {
                package_or_output.clone().map(PathBuf::from)
            },
            cache_file: None,
            record_name: if self.merge_abc {
                items
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| default_record_name(Path::new(path)))
            } else {
                String::new()
            },
            source_file,
            package_name: if self.merge_abc {
                package_or_output
            } else {
                None
            },
            is_commonjs,
            is_shared_module: items.get(5).is_some_and(|value| value == "true"),
            origin_source_lang: items.get(6).cloned().unwrap_or_default(),
            source_map: None,
        };
        self.inputs.push(InputSpec::FileInfo(input));
        Ok(())
    }

    fn apply_cache_file_mapping(&self, inputs: &mut [Ts2abcInput]) -> Result<(), Ts2abcError> {
        let Some(cache_file) = &self.cache_file else {
            return Ok(());
        };
        let raw = cache_file.to_string_lossy();
        if let Some(manifest) = raw.strip_prefix('@') {
            let map = CacheFileMap::read_manifest(Path::new(manifest)).map_err(Ts2abcError::Io)?;
            for input in inputs {
                let source_file = input.path.to_string_lossy();
                input.cache_file = map.get(&source_file).map(PathBuf::from);
            }
            return Ok(());
        }

        if inputs.len() == 1 {
            inputs[0].cache_file = Some(cache_file.clone());
        }
        Ok(())
    }

    fn compiler_output(&self) -> PathBuf {
        self.output
            .clone()
            .or_else(|| {
                self.inputs.iter().find_map(|input| match input {
                    InputSpec::Path(_) => None,
                    InputSpec::FileInfo(input) => input.output.clone(),
                })
            })
            .or_else(|| {
                self.default_output_source
                    .as_ref()
                    .map(|path| derived_output_path(path))
            })
            .unwrap_or_else(|| PathBuf::from("Base64Output.abc"))
    }
}

fn split_flag_value(arg: &str) -> (&str, Option<String>) {
    if let Some((flag, value)) = arg.split_once('=') {
        (flag, Some(value.to_string()))
    } else {
        (arg, None)
    }
}

fn split_file_info_items(line: &str) -> Vec<String> {
    line.split(';')
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_bool_flag(
    flag: &str,
    inline: Option<String>,
    default_when_present: bool,
) -> Result<bool, Ts2abcError> {
    let Some(value) = inline else {
        return Ok(default_when_present);
    };
    match value.as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(Ts2abcError::InvalidValue {
            flag: flag.to_string(),
            value,
        }),
    }
}

fn derived_output_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("output");
    PathBuf::from(format!("{stem}.abc"))
}
