use std::path::PathBuf;

use crate::driver::{default_record_name, Ts2abcInput, Ts2abcOptions};
use crate::error::Ts2abcError;
use crate::version::{
    ts2abc_script_target_bc_version, BytecodeVersion, DEFAULT_TARGET_API_VERSION,
};

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

pub fn parse_ts2abc_args<I, S>(args: I) -> Result<Ts2abcCommand, Ts2abcError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut parser = ArgParser::new(args.into_iter().map(Into::into).collect());
    parser.parse()
}

struct ArgParser {
    args: Vec<String>,
    index: usize,
    target_api_version: u32,
    target_api_sub_version: Option<String>,
    output: Option<PathBuf>,
    record_name: Option<String>,
    source_file: Option<String>,
    cache_file: Option<PathBuf>,
    debug_info: bool,
    merge_abc: bool,
    module: bool,
    commonjs: bool,
    opt_level: u8,
    print_bc_version: bool,
    print_bc_min_version: bool,
    print_target_bc_version: bool,
    inputs: Vec<PathBuf>,
}

impl ArgParser {
    fn new(args: Vec<String>) -> Self {
        Self {
            args,
            index: 0,
            target_api_version: DEFAULT_TARGET_API_VERSION,
            target_api_sub_version: None,
            output: None,
            record_name: None,
            source_file: None,
            cache_file: None,
            debug_info: false,
            merge_abc: false,
            module: false,
            commonjs: false,
            opt_level: 2,
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
            let version = ts2abc_script_target_bc_version(
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

        let output = self.output.clone().ok_or(Ts2abcError::MissingOutput)?;
        if self.inputs.is_empty() {
            return Err(Ts2abcError::MissingInput);
        }

        let single_record_name = self.record_name.clone();
        let inputs = self
            .inputs
            .iter()
            .map(|path| Ts2abcInput {
                path: path.clone(),
                record_name: single_record_name
                    .clone()
                    .unwrap_or_else(|| default_record_name(path)),
                source_file: self.source_file.clone(),
                source_map: None,
            })
            .collect();

        Ok(Ts2abcCommand::Compile(Ts2abcOptions {
            inputs,
            output,
            target_api_version: self.target_api_version,
            target_api_sub_version: self.target_api_sub_version.clone(),
            debug_info: self.debug_info,
            opt_level: self.opt_level,
            merge_abc: self.merge_abc,
            module: self.module,
            commonjs: self.commonjs,
            cache_file: self.cache_file.clone(),
        }))
    }

    fn parse_one(&mut self, arg: String) -> Result<(), Ts2abcError> {
        if arg == "--" {
            while self.index < self.args.len() {
                self.inputs.push(PathBuf::from(&self.args[self.index]));
                self.index += 1;
            }
            return Ok(());
        }
        if arg.starts_with('@') {
            return Err(Ts2abcError::UnsupportedFlag(arg));
        }
        if !arg.starts_with("--") {
            self.inputs.push(PathBuf::from(arg));
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
            "--debug-info" => self.debug_info = true,
            "--merge-abc" => self.merge_abc = true,
            "--module" => self.module = true,
            "--commonjs" => self.commonjs = true,
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
            "--file-threads"
            | "--function-threads"
            | "--abc-class-threads"
            | "--perf-file"
            | "--perf-level" => {
                let _ = self.value(flag, value)?;
            }
            "--enable-release-column" | "--use-define-semantic" => {}
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
}

fn split_flag_value(arg: &str) -> (&str, Option<String>) {
    if let Some((flag, value)) = arg.split_once('=') {
        (flag, Some(value.to_string()))
    } else {
        (arg, None)
    }
}
