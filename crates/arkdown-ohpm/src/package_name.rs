//! OHPM package name type with validation and disk-encoding helpers.
//!
//! OHPM package names follow npm naming conventions:
//! - Plain packages: `"lodash"`, `"some-lib"`
//! - Scoped packages: `"@scope/name"`, `"@ohos/hilog"`
//!
//! # Disk encoding
//!
//! Scoped package names use `@scope/name` in manifest files but are stored on
//! disk as `@scope+name` to avoid creating nested directory structures.

use std::fmt;

use thiserror::Error;

/// An OHPM package name.
///
/// Wraps the raw name string and provides disk-encoding helpers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageName(Box<str>);

/// Errors produced when parsing a package name.
#[derive(Debug, Error)]
pub enum PackageNameError {
    /// The name is empty.
    #[error("package name must not be empty")]
    Empty,
    /// The scoped name is missing the name part after `/`.
    #[error("scoped package name {0:?} must have a name after '/'")]
    MissingScopedName(String),
}

impl PackageName {
    /// Parse a package name string.
    ///
    /// # Errors
    ///
    /// - [`PackageNameError::Empty`] — empty string.
    /// - [`PackageNameError::MissingScopedName`] — `@scope/` with no name.
    pub fn new(s: impl Into<Box<str>>) -> Result<Self, PackageNameError> {
        let s = s.into();
        if s.is_empty() {
            return Err(PackageNameError::Empty);
        }
        if let Some(scope) = s.strip_prefix('@') {
            if !scope.contains('/') || scope.ends_with('/') {
                return Err(PackageNameError::MissingScopedName(s.into()));
            }
        }
        Ok(Self(s))
    }

    /// Construct without validation (for trusted inputs).
    pub fn from_raw(s: impl Into<Box<str>>) -> Self {
        Self(s.into())
    }

    /// Borrow as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return `true` if this is a scoped package (`@scope/name`).
    pub fn is_scoped(&self) -> bool {
        self.0.starts_with('@')
    }

    /// Return the disk-safe encoding of the package name.
    ///
    /// `@scope/name` → `@scope+name`
    /// `plain-name` → `plain-name`
    ///
    /// This matches the OHPM global cache directory naming convention.
    pub fn disk_name(&self) -> String {
        if self.is_scoped() {
            // Replace the first `/` (which follows the scope) with `+`.
            self.0.replacen('/', "+", 1)
        } else {
            self.0.to_string()
        }
    }

    /// Return the scope prefix, if any.
    ///
    /// For `@ohos/hilog` this returns `Some("@ohos")`.
    pub fn scope(&self) -> Option<&str> {
        if let Some(rest) = self.0.strip_prefix('@') {
            let slash_pos = rest.find('/')?;
            Some(&self.0[..slash_pos + 1])
        } else {
            None
        }
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for PackageName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<PackageName> for String {
    fn from(n: PackageName) -> Self {
        n.0.into()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_package_name() {
        let n = PackageName::new("lodash").unwrap();
        assert_eq!(n.as_str(), "lodash");
        assert!(!n.is_scoped());
        assert_eq!(n.disk_name(), "lodash");
        assert_eq!(n.scope(), None);
    }

    #[test]
    fn test_scoped_package_name() {
        let n = PackageName::new("@ohos/hilog").unwrap();
        assert_eq!(n.as_str(), "@ohos/hilog");
        assert!(n.is_scoped());
        assert_eq!(n.disk_name(), "@ohos+hilog");
        assert_eq!(n.scope(), Some("@ohos"));
    }

    #[test]
    fn test_scoped_with_hyphenated_name() {
        let n = PackageName::new("@scope/some-lib").unwrap();
        assert_eq!(n.disk_name(), "@scope+some-lib");
    }

    #[test]
    fn test_empty_name_rejected() {
        assert!(PackageName::new("").is_err());
    }

    #[test]
    fn test_scoped_missing_name_rejected() {
        assert!(PackageName::new("@scope/").is_err());
    }

    #[test]
    fn test_scoped_no_slash_rejected() {
        // `@scope` without a `/name` part is malformed.
        assert!(PackageName::new("@scope").is_err());
    }

    #[test]
    fn test_disk_name_plain_unchanged() {
        let n = PackageName::new("some-library").unwrap();
        assert_eq!(n.disk_name(), "some-library");
    }
}
