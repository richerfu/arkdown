//! ArkTS language-version selection shared by the build session and compiler.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ArkTS source language selected for one arkdown build invocation.
///
/// The selected product owns the default language version, matching hvigor's
/// `targetData.getProduct().arkTSVersion` handoff. A typed build request may
/// explicitly override it; source extensions and directives never do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArktsLanguageVersion {
    /// Dynamic ArkTS / ArkTS 1.1.
    #[default]
    #[serde(rename = "1.1")]
    V1_1,
    /// Static ArkTS / ArkTS 1.2.
    #[serde(rename = "1.2")]
    V1_2,
}

impl ArktsLanguageVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1_1 => "1.1",
            Self::V1_2 => "1.2",
        }
    }

    pub const fn is_static(self) -> bool {
        matches!(self, Self::V1_2)
    }

    /// Merged bytecode filename owned by the selected frontend.
    ///
    /// The ArkTS 1.2 SDK driver declares `modules_static.abc` in
    /// `driver/build_system/src/pre_define.ts::MERGED_ABC_FILE`; the dynamic
    /// ets2abc pipeline retains `modules.abc`.
    pub const fn merged_abc_file_name(self) -> &'static str {
        match self {
            Self::V1_1 => "modules.abc",
            Self::V1_2 => "modules_static.abc",
        }
    }
}

impl fmt::Display for ArktsLanguageVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ArktsLanguageVersion {
    type Err = ParseArktsLanguageVersionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "1.1" => Ok(Self::V1_1),
            "1.2" => Ok(Self::V1_2),
            _ => Err(ParseArktsLanguageVersionError {
                value: value.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unsupported ArkTS language version '{value}'; expected '1.1' or '1.2'")]
pub struct ParseArktsLanguageVersionError {
    value: Box<str>,
}
