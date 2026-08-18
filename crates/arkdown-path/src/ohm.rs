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
//! @normalized:<isSO>&<moduleName>&<bundleName>&<normalizedImport>&<version>
//! ```
//!
//! These are **not** filesystem paths. They are runtime identifiers emitted by
//! the bundler into generated JavaScript and `loader.json`.

use std::borrow::Cow;
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

/// Whether `s` starts with a strict, parseable [`OhmUrl`] schema prefix
/// (`@bundle:`, `@shared:`, `@module:`, `@normalized:`).
///
/// Single source of truth for [`OhmUrl::parse`], [`OhmUrl::schema`], and
/// specifier classification — do not hand-write `starts_with` chains for
/// this set.
pub fn is_ohm_url_prefix(s: &str) -> bool {
    s.starts_with("@bundle:")
        || s.starts_with("@shared:")
        || s.starts_with("@module:")
        || s.starts_with(OhmUrl::NORMALIZED_PREFIX)
}

/// Whether `s` is an OHM-style runtime URL the bundler passes through
/// untouched.
///
/// Deliberate superset of [`is_ohm_url_prefix`]: hvigor's module-source
/// rewrite also treats the package/app registry prefixes (`@package:`,
/// `@app:`) and the SDK system prefixes (`@ohos:`, `@hms:`, `@native:`) as
/// already-resolved runtime URLs. These extras are NOT parseable
/// [`OhmUrl`]s — code that needs schema access must go through
/// [`OhmUrl::parse`], which stays strict.
pub fn is_ohm_runtime_specifier(s: &str) -> bool {
    is_ohm_url_prefix(s)
        || s.starts_with("@package:")
        || s.starts_with("@app:")
        || s.starts_with("@ohos:")
        || s.starts_with("@hms:")
        || s.starts_with("@native:")
}

/// Parsed `@shared:<bundleName>/<moduleName>` reference parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OhmSharedParts<'a> {
    pub bundle_name: &'a str,
    pub module_name: &'a str,
}

/// Parsed `@bundle:<bundleName>/<moduleName>/<path>` reference parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OhmBundleParts<'a> {
    pub bundle_name: &'a str,
    pub module_name: &'a str,
    pub path: &'a str,
}

/// Parsed normalized OHM URL fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedOhmUrl<'a> {
    raw: &'a str,
    fields: Vec<&'a str>,
}

/// Errors produced when building or parsing an [`OhmUrl`].
#[derive(Debug, Error)]
pub enum OhmUrlError {
    /// The string does not start with a recognized OHM schema.
    #[error("unrecognized OHM URL schema: {0}")]
    UnrecognizedSchema(String),
}

impl OhmUrl {
    pub const NORMALIZED_PREFIX: &'static str = "@normalized:";
    pub const NORMALIZED_NOT_SO_PREFIX: &'static str = "@normalized:N";

    /// Construct an `@bundle:` URL.
    ///
    /// ```text
    /// @bundle:<bundle_name>/<module_name>/<rel_path>
    /// ```
    pub fn bundle(bundle_name: &str, module_name: &str, rel_path: &str) -> Self {
        Self(Self::slash_url(
            "@bundle:",
            &[bundle_name, module_name, rel_path],
        ))
    }

    /// Construct a package-level `@bundle:` URL used by bytecode HAR external
    /// references.
    pub fn bundle_package(package_name: &str) -> Self {
        Self(Self::prefixed("@bundle:", package_name))
    }

    /// Construct an `@shared:` URL.
    ///
    /// ```text
    /// @shared:<hsp_name>/<rel_path>
    /// ```
    pub fn shared(hsp_name: &str, rel_path: &str) -> Self {
        Self(Self::slash_url("@shared:", &[hsp_name, rel_path]))
    }

    /// Construct an `@module:` URL.
    ///
    /// ```text
    /// @module:<module_name>/<rel_path>
    /// ```
    pub fn module(module_name: &str, rel_path: &str) -> Self {
        Self(Self::slash_url("@module:", &[module_name, rel_path]))
    }

    /// Construct a non-SO normalized OHM URL.
    ///
    /// ```text
    /// @normalized:N&<module_name>&<bundle_name>&<normalized_import>&<version>
    /// ```
    pub fn normalized_non_so(
        module_name: &str,
        bundle_name: &str,
        normalized_import: &str,
        version: &str,
    ) -> Self {
        Self(Self::ampersand_url(
            Self::NORMALIZED_NOT_SO_PREFIX,
            &[module_name, bundle_name, normalized_import, version],
        ))
    }

    /// Construct a SO normalized OHM URL.
    ///
    /// ```text
    /// @normalized:Y&<module_name>&<bundle_name>&<normalized_import>&<version>
    /// ```
    pub fn normalized_so(
        module_name: &str,
        bundle_name: &str,
        normalized_import: &str,
        version: &str,
    ) -> Self {
        Self(Self::ampersand_url(
            "@normalized:Y",
            &[module_name, bundle_name, normalized_import, version],
        ))
    }

    /// Construct a normalized OHM URL with an explicit SO flag.
    ///
    /// Same field order as [`OhmUrl::normalized_non_so`] and
    /// [`OhmUrl::normalized_so`]:
    /// `<is_so>&<module_name>&<bundle_name>&<normalized_import>&<version>`.
    pub fn normalized(
        is_so: bool,
        module_name: &str,
        bundle_name: &str,
        normalized_import: &str,
        version: &str,
    ) -> Self {
        let prefix = if is_so {
            "@normalized:Y"
        } else {
            Self::NORMALIZED_NOT_SO_PREFIX
        };
        Self(Self::ampersand_url(
            prefix,
            &[module_name, bundle_name, normalized_import, version],
        ))
    }

    fn prefixed(prefix: &str, value: &str) -> String {
        let mut url = String::with_capacity(prefix.len() + value.len());
        url.push_str(prefix);
        url.push_str(value);
        url
    }

    fn slash_url(prefix: &str, parts: &[&str]) -> String {
        let parts_len = parts.iter().map(|part| part.len()).sum::<usize>();
        let separators = parts.len().saturating_sub(1);
        let mut url = String::with_capacity(prefix.len() + parts_len + separators);
        url.push_str(prefix);
        for (index, part) in parts.iter().enumerate() {
            if index > 0 {
                url.push('/');
            }
            url.push_str(part);
        }
        url
    }

    fn ampersand_url(prefix: &str, parts: &[&str]) -> String {
        let parts_len = parts.iter().map(|part| part.len()).sum::<usize>();
        let mut url = String::with_capacity(prefix.len() + parts_len + parts.len());
        url.push_str(prefix);
        for part in parts {
            url.push('&');
            url.push_str(part);
        }
        url
    }

    /// Parse an OHM URL from a string, validating the schema prefix.
    ///
    /// # Errors
    ///
    /// Returns [`OhmUrlError::UnrecognizedSchema`] if the string does not start
    /// with `@bundle:`, `@shared:`, `@module:`, or `@normalized:`.
    pub fn parse(s: impl Into<String>) -> Result<Self, OhmUrlError> {
        let s = s.into();
        if is_ohm_url_prefix(&s) {
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
        } else if self.0.starts_with(Self::NORMALIZED_PREFIX) {
            Some(OhmSchema::Normalized)
        } else {
            None
        }
    }

    /// Return `true` for runtime references resolved outside the current
    /// bundle graph.
    pub fn is_cross_bundle_reference(&self) -> bool {
        matches!(self.schema(), Some(OhmSchema::Bundle | OhmSchema::Shared))
    }

    /// Return true when a raw import/dependency string carries the non-SO
    /// normalized OHMURL marker used by upstream dependency traversal.
    pub fn has_normalized_not_so_marker(raw: &str) -> bool {
        raw.contains(Self::NORMALIZED_NOT_SO_PREFIX)
    }

    /// Parse a shared HSP reference into bundle and module names.
    pub fn shared_parts(&self) -> Option<OhmSharedParts<'_>> {
        let rest = self.0.strip_prefix("@shared:")?;
        let slash = rest.find('/')?;
        Some(OhmSharedParts {
            bundle_name: &rest[..slash],
            module_name: &rest[slash + 1..],
        })
    }

    /// Parse a bundle reference into bundle name, module name, and path.
    pub fn bundle_parts(&self) -> Option<OhmBundleParts<'_>> {
        Self::parse_bundle_parts(&self.0)
    }

    /// Parse a borrowed bundle reference into bundle name, module name, and
    /// path without allocating.
    pub fn parse_bundle_parts(s: &str) -> Option<OhmBundleParts<'_>> {
        let rest = s.strip_prefix("@bundle:")?;
        let slash1 = rest.find('/')?;
        let after_bundle = &rest[slash1 + 1..];
        let slash2 = after_bundle.find('/')?;
        Some(OhmBundleParts {
            bundle_name: &rest[..slash1],
            module_name: &after_bundle[..slash2],
            path: &after_bundle[slash2 + 1..],
        })
    }
}

impl<'a> NormalizedOhmUrl<'a> {
    const MODULE_NAME_POS: usize = 1;
    const BUNDLE_NAME_POS: usize = 2;
    const NORMALIZED_IMPORT_POS: usize = 3;
    const VERSION_POS: usize = 4;
    const FIELD_SEPARATOR: &'static str = "&";
    const PATH_SEPARATOR: char = '/';
    const SCOPE_MARKER: char = '@';

    /// Parse a normalized OHM URL.
    pub fn parse(raw: &'a str) -> Option<Self> {
        if !raw.starts_with(OhmUrl::NORMALIZED_PREFIX) {
            return None;
        }
        Self::from_raw(raw)
    }

    /// Parse a normalized non-SO OHM URL.
    pub fn parse_non_so(raw: &'a str) -> Option<Self> {
        if !raw.starts_with(OhmUrl::NORMALIZED_NOT_SO_PREFIX) {
            return None;
        }
        Self::from_raw(raw)
    }

    fn from_raw(raw: &'a str) -> Option<Self> {
        let fields: Vec<&str> = raw.split(Self::FIELD_SEPARATOR).collect();
        (fields.len() > Self::VERSION_POS).then_some(Self { raw, fields })
    }

    /// Extract the package name from a normalized import field.
    ///
    /// Examples:
    /// - `entry/src/main/ets/pages/Index` -> `entry`
    /// - `@scope/pkg/src/Index` -> `@scope/pkg`
    pub fn package_name_from_import(normalized_import: &'a str) -> Option<&'a str> {
        let mut pos = normalized_import.find(Self::PATH_SEPARATOR)?;
        if normalized_import.starts_with(Self::SCOPE_MARKER) {
            pos = normalized_import[pos + 1..].find(Self::PATH_SEPARATOR)? + pos + 1;
        }
        Some(&normalized_import[..pos])
    }

    /// Extract the package name from this URL.
    pub fn package_name(&self) -> Option<&'a str> {
        Self::package_name_from_import(self.normalized_import()?)
    }

    /// Extract the ABC record name represented by this URL.
    pub fn record_name(&self) -> Option<String> {
        Some(Self::join_fields([
            self.bundle_name()?,
            self.normalized_import()?,
            self.version()?,
        ]))
    }

    /// Return the bundle-name field.
    pub fn bundle_name(&self) -> Option<&'a str> {
        self.fields.get(Self::BUNDLE_NAME_POS).copied()
    }

    /// Return the module-name field.
    pub fn module_name(&self) -> Option<&'a str> {
        self.fields.get(Self::MODULE_NAME_POS).copied()
    }

    /// Return the normalized import field.
    pub fn normalized_import(&self) -> Option<&'a str> {
        self.fields.get(Self::NORMALIZED_IMPORT_POS).copied()
    }

    /// Return the package version field.
    pub fn version(&self) -> Option<&'a str> {
        self.fields.get(Self::VERSION_POS).copied()
    }

    /// Rebuild this URL with a new bundle-name field.
    pub fn with_bundle_name(&self, bundle_name: &str) -> Option<String> {
        let mut fields = self.fields.clone();
        *fields.get_mut(Self::BUNDLE_NAME_POS)? = bundle_name;
        Some(fields.join(Self::FIELD_SEPARATOR))
    }

    /// Rebuild this URL with a new package version field.
    pub fn with_version(&self, version: &str) -> Option<String> {
        let mut fields = self.fields.clone();
        *fields.get_mut(Self::VERSION_POS)? = version;
        Some(fields.join(Self::FIELD_SEPARATOR))
    }

    /// Rebuild this URL with the leading package segment in the normalized
    /// import field renamed.
    pub fn with_package_name(&self, source_package: &str, target_package: &str) -> Option<String> {
        let normalized_import = self.normalized_import()?;
        let suffix = normalized_import.strip_prefix(source_package)?;
        let mut fields = self
            .fields
            .iter()
            .map(|field| Cow::Borrowed(*field))
            .collect::<Vec<_>>();
        *fields.get_mut(Self::NORMALIZED_IMPORT_POS)? =
            Cow::Owned(Self::replace_package_prefix(target_package, suffix));
        Some(
            fields
                .iter()
                .map(|field| field.as_ref())
                .collect::<Vec<_>>()
                .join(Self::FIELD_SEPARATOR),
        )
    }

    fn join_fields<const N: usize>(fields: [&str; N]) -> String {
        let fields_len = fields.iter().map(|field| field.len()).sum::<usize>();
        let separators = N.saturating_sub(1) * Self::FIELD_SEPARATOR.len();
        let mut text = String::with_capacity(fields_len + separators);
        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                text.push_str(Self::FIELD_SEPARATOR);
            }
            text.push_str(field);
        }
        text
    }

    fn replace_package_prefix(target_package: &str, suffix: &str) -> String {
        let mut import = String::with_capacity(target_package.len() + suffix.len());
        import.push_str(target_package);
        import.push_str(suffix);
        import
    }

    /// Borrow the raw URL text.
    pub fn as_str(&self) -> &'a str {
        self.raw
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
        assert_eq!(
            url.as_str(),
            "@bundle:com.example.app/entry/ets/pages/Index"
        );
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
