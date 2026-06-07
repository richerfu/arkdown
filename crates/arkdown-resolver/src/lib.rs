//! OHOS module resolution library for arkdown.
//!
//! Maps OHOS import specifiers to resolved modules following OHOS-specific
//! resolution rules: relative imports, SDK APIs (`@ohos/`, `@kit.`), external
//! cross-bundle references (`@bundle:`, `@shared:`), and `oh_modules/` packages.
//!
//! # SPEC-048 file structure target
//!
//! | File              | Responsibility                                     |
//! |-------------------|----------------------------------------------------|
//! | `resolver.rs`     | `OhosResolver` over resolver-owned indexes         |
//! | `specifier.rs`    | Specifier parsing and classification               |
//! | `ohm_url.rs`      | OHM URL builder and parser                         |
//! | `case_check.rs`   | Case-sensitive path verification                   |
//! | `error.rs`        | Resolver errors                                    |
//! | `oh_modules.rs`   | Keep for test fixtures; hot-path scan deleted      |
//! | `sdk.rs`          | Delete resolver-owned SDK probing (phase 3)        |

pub mod case_check;
pub mod error;
pub mod oh_modules;
pub mod ohm_url;
pub mod resolver;
pub mod sdk;
pub mod specifier;

pub use case_check::{check_path_case, CaseCheckResult};
pub use error::ResolveError;
pub use oh_modules::OhPackage;
pub use ohm_url::{generate_ohm_url, parse_ohm_url};
pub use resolver::{OhosResolver, PkgBriefInfo, ResolverConfig};
pub use specifier::{classify_specifier, SpecifierClass};
