//! Raw OHOS JSON/JSON5 schema types for the arkdown bundler.
//!
//! This crate owns only the deserialization structs for OHOS project files.
//! It has no filesystem discovery, no processing policy, and no session
//! lifecycle. Callers supply parsed strings; this crate deserializes them.
//!
//! # Contents
//!
//! | Module         | File                 |
//! |----------------|----------------------|
//! | [`app_json`]   | `AppScope/app.json5` |
//! | [`oh_package`] | `oh-package.json5`   |

pub mod app_json;
pub mod ets_options;
pub mod oh_package;

pub use app_json::{RawAppJson5, RawAppJson5App};
pub use oh_package::RawOhPackage;
