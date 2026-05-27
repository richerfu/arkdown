//! Profile resource handling.
//!
//! Profile files (router map JSON, startup config, etc.) are copied verbatim
//! into `<output>/base/profile/`.  The copy logic lives inline in
//! [`crate::compiler::ResourceCompiler::compile`]; this module is the
//! designated home for any future profile-specific validation or transformation
//! (e.g. schema validation of router map entries).
