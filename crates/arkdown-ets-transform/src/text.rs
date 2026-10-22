//! Text conversions for OXC 0.155 string types.
//!
//! OXC now stores JavaScript string literals as [`JSStr`], which is WTF-8 and may
//! contain lone surrogates. These helpers keep the conversion explicit: names
//! used as identifiers or module specifiers cannot contain a lone surrogate, so
//! `as_str` is used where possible and diagnostics use `display`.

use oxc_ast::StaticPropertyName;
use oxc_str::JSStr;

/// Convert a statically known property name into an owned Rust string.
pub(crate) trait StaticPropertyNameText {
    /// Own the name, spelling lone surrogates as `\uXXXX` when they cannot be
    /// represented as UTF-8.
    fn to_owned_text(&self) -> String;

    /// Borrow the name as UTF-8, or an empty string for a lone surrogate.
    ///
    /// Callers use this only for matching well-known names, where a lone
    /// surrogate can never match.
    fn as_str_or_empty(&self) -> &str;
}

impl StaticPropertyNameText for StaticPropertyName<'_> {
    fn to_owned_text(&self) -> String {
        self.as_str()
            .map_or_else(|| self.display().to_string(), str::to_owned)
    }

    fn as_str_or_empty(&self) -> &str {
        self.as_str().unwrap_or_default()
    }
}

/// Convert a JavaScript string literal value into an owned Rust string.
pub(crate) trait JsStrText {
    /// Own the value, spelling lone surrogates as `\uXXXX`.
    fn to_owned_text(&self) -> String;
}

impl JsStrText for JSStr<'_> {
    fn to_owned_text(&self) -> String {
        self.display().to_string()
    }
}
