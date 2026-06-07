//! [`OhmUrl`] — OHOS runtime module URL.
//!
//! OHM URLs appear in:
//! - The `globalThis` module registry keys.
//! - `loader.json` module path maps and HAR/HSP maps.
//! - Cross-HAR and cross-HSP import references.
//!
//! # Formats
//!
//! ```text
//! @bundle:<bundleName>/<moduleName>/<path>
//! @shared:<hspName>/<path>
//! @module:<moduleName>/<path>
//! @normalized:<extra>/<bundleName>/<moduleName>/<path>&<queryParams>
//! ```
//!
//! These are **not** filesystem paths. They are runtime identifiers emitted by
//! the bundler into generated JavaScript and `loader.json`.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A runtime OHM URL.
///
/// Do **not** perform filesystem operations using this type. Use it only to
/// pass around runtime module identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OhmUrl(String);

/// The schema prefix of an OHM URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OhmSchema {
    /// `@bundle:<bundleName>/…`
    Bundle,
    /// `@shared:<hspName>/…`
    Shared,
    /// `@module:<moduleName>/…`
    Module,
    /// `@normalized:…`
    Normalized,
}

/// Errors produced when building or parsing an [`OhmUrl`].
#[derive(Debug, Error)]
pub enum OhmUrlError {
    /// The string does not start with a recognized OHM schema.
    #[error("unrecognized OHM URL schema: {0}")]
    UnrecognizedSchema(String),
}

impl OhmUrl {
    /// Construct an `@bundle:` URL.
    ///
    /// ```text
    /// @bundle:<bundle_name>/<module_name>/<rel_path>
    /// ```
    pub fn bundle(bundle_name: &str, module_name: &str, rel_path: &str) -> Self {
        Self(format!("@bundle:{bundle_name}/{module_name}/{rel_path}"))
    }

    /// Construct an `@shared:` URL.
    ///
    /// ```text
    /// @shared:<hsp_name>/<rel_path>
    /// ```
    pub fn shared(hsp_name: &str, rel_path: &str) -> Self {
        Self(format!("@shared:{hsp_name}/{rel_path}"))
    }

    /// Construct an `@module:` URL.
    ///
    /// ```text
    /// @module:<module_name>/<rel_path>
    /// ```
    pub fn module(module_name: &str, rel_path: &str) -> Self {
        Self(format!("@module:{module_name}/{rel_path}"))
    }

    /// Parse an OHM URL from a string, validating the schema prefix.
    ///
    /// # Errors
    ///
    /// Returns [`OhmUrlError::UnrecognizedSchema`] if the string does not start
    /// with `@bundle:`, `@shared:`, `@module:`, or `@normalized:`.
    pub fn parse(s: impl Into<String>) -> Result<Self, OhmUrlError> {
        let s = s.into();
        if s.starts_with("@bundle:")
            || s.starts_with("@shared:")
            || s.starts_with("@module:")
            || s.starts_with("@normalized:")
        {
            Ok(Self(s))
        } else {
            Err(OhmUrlError::UnrecognizedSchema(s))
        }
    }

    /// Construct without validation.
    pub fn from_raw(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the inner `String`.
    pub fn into_string(self) -> String {
        self.0
    }

    /// Return the schema prefix.
    pub fn schema(&self) -> Option<OhmSchema> {
        if self.0.starts_with("@bundle:") {
            Some(OhmSchema::Bundle)
        } else if self.0.starts_with("@shared:") {
            Some(OhmSchema::Shared)
        } else if self.0.starts_with("@module:") {
            Some(OhmSchema::Module)
        } else if self.0.starts_with("@normalized:") {
            Some(OhmSchema::Normalized)
        } else {
            None
        }
    }
}

impl fmt::Display for OhmUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for OhmUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ohm_bundle_url() {
        let url = OhmUrl::bundle("com.example.app", "entry", "ets/pages/Index");
        assert_eq!(url.as_str(), "@bundle:com.example.app/entry/ets/pages/Index");
        assert_eq!(url.schema(), Some(OhmSchema::Bundle));
    }

    #[test]
    fn test_ohm_shared_url() {
        let url = OhmUrl::shared("hsp-lib", "ets/Index");
        assert_eq!(url.as_str(), "@shared:hsp-lib/ets/Index");
        assert_eq!(url.schema(), Some(OhmSchema::Shared));
    }

    #[test]
    fn test_ohm_module_url() {
        let url = OhmUrl::module("entry", "ets/utils/helper");
        assert_eq!(url.as_str(), "@module:entry/ets/utils/helper");
        assert_eq!(url.schema(), Some(OhmSchema::Module));
    }

    #[test]
    fn test_ohm_parse_valid() {
        let url = OhmUrl::parse("@bundle:com.foo/bar/ets/Index").unwrap();
        assert_eq!(url.schema(), Some(OhmSchema::Bundle));
    }

    #[test]
    fn test_ohm_parse_invalid() {
        let err = OhmUrl::parse("@ohos/hilog");
        assert!(err.is_err());
    }

    #[test]
    fn test_ohm_parse_normalized() {
        let url = OhmUrl::parse("@normalized:N&&&entry&ets/Index").unwrap();
        assert_eq!(url.schema(), Some(OhmSchema::Normalized));
    }

    #[test]
    fn test_ohm_display() {
        let url = OhmUrl::bundle("pkg", "mod", "ets/Index");
        assert_eq!(url.to_string(), "@bundle:pkg/mod/ets/Index");
    }
}
