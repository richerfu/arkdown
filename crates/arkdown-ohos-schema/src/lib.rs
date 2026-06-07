//! Raw OHOS JSON/JSON5 schema types for the arkdown bundler.
//!
//! This crate owns only the deserialization structs for OHOS project files.
//! It has no filesystem discovery, no processing policy, and no session
//! lifecycle. Callers supply parsed strings; this crate deserializes them.
//!
//! # Contents
//!
//! | Module             | File                          |
//! |--------------------|-------------------------------|
//! | [`app_json`]       | `AppScope/app.json5`          |
//! | [`build_profile`]  | `build-profile.json5`         |
//! | [`module_json`]    | `src/main/module.json5`       |
//! | [`oh_package`]     | `oh-package.json5`            |
//! | [`profiles`]       | Profile JSON files            |
//! | [`loader_json`]    | `loader.json`                 |
//! | [`pack_info`]      | `pack.info`                   |
//! | [`signing`]        | Signing config/profile        |

pub mod app_json;
pub mod build_profile;
pub mod loader_json;
pub mod module_json;
pub mod oh_package;
pub mod pack_info;
pub mod profiles;
pub mod signing;

pub use app_json::{RawAppJson5, RawAppJson5App};
pub use build_profile::{
    RawApp, RawBuildProfile, RawBuildProfileModule, RawBuildProfileProduct,
    RawBuildProfileSigningConfig,
};
pub use module_json::{RawModuleEntry, RawModuleJson};
pub use oh_package::RawOhPackage;
