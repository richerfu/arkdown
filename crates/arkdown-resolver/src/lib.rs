//! OHOS module resolution library for arkdown.
//!
//! Maps OHOS import specifiers to resolved modules following OHOS-specific
//! resolution rules: relative imports, SDK APIs (`@ohos/`, `@kit.`), external
//! cross-bundle references (`@bundle:`, `@shared:`), and `oh_modules/` packages.
//!
//! This library does NOT implement [`ArkdownPlugin`]; that is SPEC-015.

pub mod error;
pub mod oh_modules;
pub mod ohm_url;
pub mod resolver;
pub mod sdk;

pub use error::ResolveError;
pub use oh_modules::OhPackage;
pub use ohm_url::{generate_ohm_url, parse_ohm_url};
pub use resolver::{OhosResolver, PkgBriefInfo, ResolverConfig};
