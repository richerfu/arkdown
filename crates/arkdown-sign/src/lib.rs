//! Arkdown's OpenHarmony signing control layer.
//!
//! Project configuration and DevEco password material are adapted here. The
//! signing formats, keystore handling, CMS generation, archive alignment,
//! code-sign blocks, and atomic output replacement are owned by the published
//! `hapsigner` crate.

pub mod backend;
mod config_material;
pub mod error;
pub mod plan;

pub use backend::{HarSignBackendRunner, SignBackendRunner};
pub use error::SignError;
pub use plan::{SignPlan, SigningMaterial, SigningPassword};
