//! Narrow plan input views for the application `BuildPlanBuilder`.
//!
//! Each `*PlanInput` type provides a narrow, stage-specific view of the
//! session context for use by domain crates.  Domain crates receive these
//! typed inputs instead of raw session access.
//!
//! # Module layout
//!
//! - [`profile`] — `ProfilePlanInput` for the profile processing stage.
//! - [`resource`] — `ResourcePlanInput` for the resource compilation stage.
//! - [`bundle`] — `BundlePlanInput` for the rolldown bundle stage.
//! - [`bytecode`] — `BytecodePlanInput` for the ABC compilation stage.
//! - [`pack`] — `PackagePlanInput` for the HAP/HSP/HAR packaging stage.
//! - [`sign`] — `SignPlanInput` for the signing stage.
//! - [`native`] — `NativePlanInput` for the native C/C++ build stage.
//! - [`watch`] — `WatchPlanInput` for the filesystem watch stage.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `ProfilePlanInput` | `ProcessProfile` task inputs |
//! | `ResourcePlanInput` | `CompileResource` task inputs |
//! | `BundlePlanInput` | `ArkCompile` rolldown config inputs |
//! | `BytecodePlanInput` | `CompileArkTS` / `es2abc` invocation inputs |
//! | `PackagePlanInput` | `PackHAP`/`PackHSP`/`PackHAR` task inputs |
//! | `SignPlanInput` | `SignHap` task inputs |
//! | `NativePlanInput` | `NativeBuild` task inputs |
//! | `WatchPlanInput` | watch mode configuration inputs |

pub mod bundle;
pub mod bytecode;
pub mod native;
pub mod pack;
pub mod profile;
pub mod resource;
pub mod sign;
pub mod watch;

// Flat re-exports so callers can use `plan_input::BundlePlanInput` etc.
pub use bundle::BundlePlanInput;
pub use bytecode::BytecodePlanInput;
pub use native::NativePlanInput;
pub use pack::PackagePlanInput;
pub use profile::ProfilePlanInput;
pub use resource::ResourcePlanInput;
pub use sign::SignPlanInput;
pub use watch::WatchPlanInput;
