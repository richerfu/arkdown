//! Media resource handling.
//!
//! Media files (images, audio, video) are copied verbatim into the output
//! directory and assigned a resource ID.  The copy logic lives inline in
//! [`crate::compiler::ResourceCompiler::compile`]; this module exists as the
//! designated home for any future media-specific processing (e.g. image
//! optimisation, format conversion).
