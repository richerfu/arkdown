//! OHOS module resolution plugin.
//!
//! Wraps `arkdown-resolver`'s `OhosResolver` as a rolldown `Plugin`, handling
//! OHM URL resolution, `@ohos`/`@kit` SDK imports, `oh_modules` lookups, and
//! optional case-sensitivity checking.
//!
//! Previously the standalone `arkdown-plugin-ohos-resolve` crate.

pub mod importee;
pub mod plugin;

pub use importee::{ProcessImporteeConfig, ProcessImporteeResult, ProcessImporteeRunner};
pub use plugin::OhosResolvePlugin;
