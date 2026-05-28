use std::fmt;

/// Upstream es2abc defaults this argument to the current compiler API.
pub const DEFAULT_TARGET_API_VERSION: u32 = 20;

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

/// Version mapping for native `es2abc --target-bc-version`.
///
/// This follows the OpenHarmony `arkcompiler_ets_frontend` runner expectations
/// and `arkcompiler_runtime_core` `GetVersionByApi` behavior. API values newer
/// than the known map resolve to the current bytecode version.
pub fn es2abc_target_bc_version(api: u32, sub_api: Option<&str>) -> BytecodeVersion {
    match api {
        9 | 10 => BytecodeVersion::new(9, 0, 0, 0),
        11 => BytecodeVersion::new(11, 0, 2, 0),
        12 if matches!(sub_api, None | Some("beta1") | Some("beta2")) => {
            BytecodeVersion::new(12, 0, 2, 0)
        }
        12..=17 => BytecodeVersion::new(12, 0, 6, 0),
        _ => BytecodeVersion::new(13, 0, 1, 0),
    }
}

/// Version mapping exposed by `ts2abc.js`.
///
/// The JS wrapper special-cases API 8 for old IDE compatibility before
/// forwarding other requests to native `es2abc --target-bc-version`.
pub fn ts2abc_script_target_bc_version(api: u32, sub_api: Option<&str>) -> BytecodeVersion {
    if api == 8 {
        BytecodeVersion::new(0, 0, 0, 2)
    } else {
        es2abc_target_bc_version(api, sub_api)
    }
}
