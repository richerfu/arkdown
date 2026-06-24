//! JSON/JSON5 import transform plugin.
//!
//! Converts `import data from './config.json5'` into an ESM module that
//! re-exports the parsed object, so ArkCompiler receives valid JS/TS source.
//!
//! Previously the standalone `arkdown-plugin-json5` crate.

mod module_emit;
pub mod plugin;

pub use plugin::Json5Plugin;
