//! ArkTS language-version selection shared by the build session and compiler.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ArkTS source language selected for one arkdown build invocation.
///
/// The selection is intentionally explicit. Source extensions, directives,
/// manifests, and dependency metadata must not silently switch the project to
/// ArkTS 1.2. This keeps existing builds on the ArkTS 1.1 pipeline unless the
/// caller opts in with the public build parameter.
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
