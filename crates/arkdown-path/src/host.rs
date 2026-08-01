//! Host-platform path conventions used at external-tool boundaries.

/// Host operating-system family relevant to executable file names.
///
/// The value can be supplied explicitly in tests so Windows, macOS, and Linux
/// SDK layouts are protected even when the test itself runs on another host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPlatform {
    Windows,
    Macos,
    Linux,
    OtherUnix,
}

impl HostPlatform {
    /// The platform for the currently compiled arkdown binary.
    pub const CURRENT: Self = if cfg!(target_os = "windows") {
        Self::Windows
    } else if cfg!(target_os = "macos") {
        Self::Macos
    } else if cfg!(target_os = "linux") {
        Self::Linux
    } else {
        Self::OtherUnix
    };

    /// Return an executable file name for this host.
    ///
    /// Callers pass the suffix-free tool name. Windows SDK distributions use
    /// explicit `.exe` names, while Unix hosts use the name unchanged.
    pub fn executable_name(self, stem: &str) -> String {
        if self == Self::Windows {
            format!("{stem}.exe")
        } else {
            stem.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HostPlatform;

    #[test]
    fn executable_names_cover_all_supported_sdk_hosts() {
        assert_eq!(
            HostPlatform::Windows.executable_name("es2abc"),
            "es2abc.exe"
        );
        assert_eq!(HostPlatform::Macos.executable_name("restool"), "restool");
        assert_eq!(HostPlatform::Linux.executable_name("cmake"), "cmake");
        assert_eq!(HostPlatform::OtherUnix.executable_name("node"), "node");
    }
}
