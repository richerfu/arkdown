//! Narrow plan input for the HAP/HSP signing stage.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// SignPlanInput
// ---------------------------------------------------------------------------

/// Narrow input view for the HAP/HSP signing stage.
///
/// Contains the inputs for `arkdown-sign::SignStage`.
///
/// # Upstream correspondence
///
/// Corresponds to `SignHap` task inputs in hvigor.
#[derive(Debug, Clone)]
pub struct SignPlanInput {
    /// Path to the unsigned package.
    pub unsigned_input: PathBuf,
    /// Path for the signed output.
    pub signed_output: PathBuf,
    /// Path to the signing keystore (`.p12`).
    pub keystore_path: Option<PathBuf>,
    /// Keystore alias.
    pub key_alias: Option<String>,
    /// Profile certificate path.
    pub profile_path: Option<PathBuf>,
    /// Whether to produce a debug-signed package.
    pub is_debug: bool,
}

impl SignPlanInput {
    /// Create from pre-resolved paths.
    pub fn from_context_input(
        unsigned_input: PathBuf,
        signed_output: PathBuf,
        keystore_path: Option<PathBuf>,
        key_alias: Option<String>,
        profile_path: Option<PathBuf>,
        is_debug: bool,
    ) -> Self {
        Self {
            unsigned_input,
            signed_output,
            keystore_path,
            key_alias,
            profile_path,
            is_debug,
        }
    }
}
