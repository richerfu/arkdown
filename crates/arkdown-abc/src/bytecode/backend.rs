//! Bytecode backend trait and implementations.
//!
//! The bytecode backend abstraction decouples the `Ts2abcCompiler` from the
//! native SDK `es2abc` process.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `BytecodeBackend` trait | SDK `load-sdk-functions.js::genAbc` dispatch |
//! | `SdkEs2abcBackend` | native SDK `es2abc` subprocess |

use std::path::PathBuf;
use std::sync::Arc;

use arkdown_session::SdkEs2abcOptionSnapshot;

use crate::bytecode::driver::{Ts2abcOptions, Ts2abcOutput};
use crate::bytecode::error::Ts2abcError;

// ---------------------------------------------------------------------------
// BytecodeBackend trait
// ---------------------------------------------------------------------------

/// Abstraction over the bytecode compilation engine.
///
/// A `BytecodeBackend` compiles JS/TS/ETS source files to ABC bytecode through
/// the SDK process boundary.
///
/// # Upstream correspondence
///
/// Corresponds to `load-sdk-functions.js::genAbc` backend dispatch in hvigor.
pub(crate) trait BytecodeBackend: Send + Sync {
    /// Compile the given options to ABC bytecode.
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError>;
}

/// The SDK `es2abc` subprocess backend.
///
/// Spawns the native `es2abc` binary from the DevEco Studio SDK.  Used when
/// the Rust backend does not yet support all required flags (e.g.,
/// `--outputProto`, specific `--enable-*` options).
///
/// # Upstream correspondence
///
/// Corresponds to `load-sdk-functions.js::genAbc` when the SDK binary path
/// is provided.
#[derive(Debug, Clone)]
pub(crate) struct SdkEs2abcBackend {
    es2abc_path: PathBuf,
    es2abc_options: Arc<SdkEs2abcOptionSnapshot>,
    input_file_list: Option<PathBuf>,
}

impl SdkEs2abcBackend {
    /// Create a new backend pointing to the given `es2abc` binary.
    pub(crate) fn new(es2abc_path: PathBuf, es2abc_options: Arc<SdkEs2abcOptionSnapshot>) -> Self {
        Self {
            es2abc_path,
            es2abc_options,
            input_file_list: None,
        }
    }

    /// Set an explicit input file list path (skips per-input filesInfo
    /// generation).
    pub(crate) fn with_input_file_list(mut self, input_file_list: Option<PathBuf>) -> Self {
        self.input_file_list = input_file_list;
        self
    }
}

impl BytecodeBackend for SdkEs2abcBackend {
    fn compile(&self, options: &Ts2abcOptions) -> Result<Ts2abcOutput, Ts2abcError> {
        // Delegate to the driver's implementation.
        let backend = crate::bytecode::driver::SdkEs2abcBackend::new(
            self.es2abc_path.clone(),
            Arc::clone(&self.es2abc_options),
        )
        .with_input_file_list(self.input_file_list.clone());
        crate::bytecode::driver::BytecodeBackend::compile(&backend, options)
    }
}
