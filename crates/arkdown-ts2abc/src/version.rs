use std::fmt;

/// Default target API version, matching upstream es2abc (options.cpp:571 sets 24).
///
/// Previously this was set to 20, which caused mismatched bytecode-version
/// selection on any build that omits `--target-api-version`.
pub const DEFAULT_TARGET_API_VERSION: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytecodeVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build: u8,
}

impl BytecodeVersion {
    pub const fn new(major: u8, minor: u8, patch: u8, build: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            build,
        }
    }
}

impl fmt::Display for BytecodeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.patch, self.build
        )
    }
}

impl BytecodeVersion {
    /// Version mapping for native `es2abc --target-bc-version`.
    ///
    /// This follows the OpenHarmony `arkcompiler_ets_frontend` runner expectations
    /// and `arkcompiler_runtime_core` `GetVersionByApi` behavior. API values newer
    /// than the known map resolve to the current bytecode version.
    pub fn for_es2abc(api: u32, sub_api: Option<&str>) -> Self {
        match api {
            9 | 10 => Self::new(9, 0, 0, 0),
            11 => Self::new(11, 0, 2, 0),
            12 if matches!(sub_api, None | Some("beta1") | Some("beta2")) => Self::new(12, 0, 2, 0),
            12..=17 => Self::new(12, 0, 6, 0),
            _ => Self::new(13, 0, 1, 0),
        }
    }

    /// Version mapping exposed by `ts2abc.js`.
    ///
    /// The JS wrapper special-cases API 8 for old IDE compatibility before
    /// forwarding other requests to native `es2abc --target-bc-version`.
    pub fn for_ts2abc_script(api: u32, sub_api: Option<&str>) -> Self {
        if api == 8 {
            Self::new(0, 0, 0, 2)
        } else {
            Self::for_es2abc(api, sub_api)
        }
    }
}

// ---------------------------------------------------------------------------
// BytecodeVersionPolicy
// ---------------------------------------------------------------------------

/// Typed policy for selecting the bytecode version from an API level.
///
/// `BytecodeVersionPolicy` owns bytecode-version selection behind named-type
/// associated methods.
///
/// # Upstream correspondence
///
/// Corresponds to `ts2abc.js --target-bc-version` version selection and
/// `es2panda/aot/options.cpp::GetVersionByApi`.
pub struct BytecodeVersionPolicy;

impl BytecodeVersionPolicy {
    /// Return the bytecode version for native `es2abc --target-bc-version`.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `arkcompiler_runtime_core::GetVersionByApi`.
    pub fn for_es2abc(api: u32, sub_api: Option<&str>) -> BytecodeVersion {
        BytecodeVersion::for_es2abc(api, sub_api)
    }

    /// Return the bytecode version exposed by the `ts2abc.js` script.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `ts2abc.js` version selection, which special-cases API 8.
    pub fn for_ts2abc_script(api: u32, sub_api: Option<&str>) -> BytecodeVersion {
        BytecodeVersion::for_ts2abc_script(api, sub_api)
    }
}

// ---------------------------------------------------------------------------
// TargetApiVersionPolicy
// ---------------------------------------------------------------------------

/// Policy for the default `target-api-version` value.
///
/// # Upstream correspondence
///
/// Corresponds to `options.cpp:571` where the default target API version is
/// set to 24.
pub struct TargetApiVersionPolicy;

impl TargetApiVersionPolicy {
    /// Return the default target API version.
    ///
    /// This matches `options.cpp:571` where the default is 24.
    pub fn default_target_api_version() -> u32 {
        DEFAULT_TARGET_API_VERSION
    }
}
