//! SDK `es2abc` command-line option capabilities.
//!
//! Hvigor loads `compile_plugin.js` from the selected SDK, so its command
//! surface changes together with that SDK's `es2abc` binary. Arkdown owns its
//! command builder and therefore snapshots the actual binary's `--help`
//! option set once during session startup.

use std::path::Path;
use std::process::Command;

/// Immutable option inventory reported by one selected SDK `es2abc` binary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SdkEs2abcOptionSnapshot {
    options: Vec<Box<str>>,
    probe_failure: Option<Box<str>>,
}

impl SdkEs2abcOptionSnapshot {
    /// Probe the selected SDK compiler once.
    pub fn inspect(es2abc_path: &Path) -> Self {
        SdkEs2abcOptionProbe::new(es2abc_path).inspect()
    }

    /// Construct a deterministic snapshot from already observed option names.
    pub fn from_options<I, S>(options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut options = options
            .into_iter()
            .filter_map(|option| Self::normalized_option(option.as_ref()).map(Box::<str>::from))
            .collect::<Vec<_>>();
        options.sort_unstable();
        options.dedup();
        Self {
            options,
            probe_failure: None,
        }
    }

    /// Whether probing produced a non-empty compiler option inventory.
    pub fn is_available(&self) -> bool {
        !self.options.is_empty()
    }

    /// Whether the selected compiler accepts an option name.
    pub fn supports(&self, option: &str) -> bool {
        let Some(option) = Self::normalized_option(option) else {
            return false;
        };
        self.options
            .binary_search_by(|candidate| candidate.as_ref().cmp(option))
            .is_ok()
    }

    /// Diagnostic captured when the compiler could not expose its option set.
    pub fn probe_failure(&self) -> Option<&str> {
        self.probe_failure.as_deref()
    }

    fn normalized_option(value: &str) -> Option<&str> {
        let value = value.trim();
        let value = value.strip_prefix("--")?;
        let end = value
            .find(|character: char| !character.is_ascii_alphanumeric() && character != '-')
            .unwrap_or(value.len());
        (end > 0).then(|| &value[..end])
    }
}

struct SdkEs2abcOptionProbe<'a> {
    es2abc_path: &'a Path,
}

impl<'a> SdkEs2abcOptionProbe<'a> {
    fn new(es2abc_path: &'a Path) -> Self {
        Self { es2abc_path }
    }

    fn inspect(&self) -> SdkEs2abcOptionSnapshot {
        match Command::new(self.es2abc_path).arg("--help").output() {
            Ok(output) => {
                let mut options = self.parse_output(&output.stdout);
                options.extend(self.parse_output(&output.stderr));
                options.sort_unstable();
                options.dedup();
                if !options.is_empty() {
                    return SdkEs2abcOptionSnapshot {
                        options,
                        probe_failure: None,
                    };
                }
                let stderr = String::from_utf8_lossy(&output.stderr);
                let detail = stderr.trim();
                SdkEs2abcOptionSnapshot {
                    options,
                    probe_failure: Some(
                        if detail.is_empty() {
                            format!(
                                "'{} --help' returned {} without an option inventory",
                                self.es2abc_path.display(),
                                output.status
                            )
                        } else {
                            format!(
                                "'{} --help' returned {}: {detail}",
                                self.es2abc_path.display(),
                                output.status
                            )
                        }
                        .into_boxed_str(),
                    ),
                }
            }
            Err(error) => SdkEs2abcOptionSnapshot {
                options: Vec::new(),
                probe_failure: Some(
                    format!(
                        "failed to execute '{} --help': {error}",
                        self.es2abc_path.display()
                    )
                    .into_boxed_str(),
                ),
            },
        }
    }

    fn parse_output(&self, output: &[u8]) -> Vec<Box<str>> {
        String::from_utf8_lossy(output)
            .lines()
            .filter_map(SdkEs2abcOptionSnapshot::normalized_option)
            .map(Box::<str>::from)
            .collect()
    }
}
