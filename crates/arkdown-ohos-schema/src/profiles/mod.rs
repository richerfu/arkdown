//! Profile JSON schema types.
//!
//! Each submodule owns the raw deserialization structs for one OHOS profile
//! file format. Filesystem loading and path resolution belong to
//! `arkdown-session::profiles::store`.

pub mod distro_filter;
pub mod forms;
pub mod permission_reason;
pub mod privacy;
pub mod router;
pub mod share;
pub mod shortcut;
pub mod startup;

pub use distro_filter::RawDistroFilterProfile;
pub use forms::RawFormProfile;
pub use permission_reason::RawPermissionReasonProfile;
pub use privacy::RawPrivacyProfile;
pub use router::RawRouterMap;
pub use share::RawShareProfile;
pub use shortcut::RawShortcutProfile;
pub use startup::RawStartupProfile;
