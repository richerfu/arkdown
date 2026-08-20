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
pub mod plan;
pub mod resolver;
pub mod specifier;

pub use case_check::{CaseCheckPolicy, CaseCheckResult};
pub use error::ResolveError;
pub use oh_modules::{OhModulesLayout, OhPackage};
pub use ohm_url::{OhmUrlBuilder, OhmUrlParseResult};
pub use plan::{PackageResolutionPlan, PackageResolutionPlanBuilder, PkgBriefInfo};
pub use resolver::{OhosResolver, ResolveImportKind, ResolverConfig};
pub use specifier::SpecifierClass;
