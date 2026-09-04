//! SDK snapshot.

mod es2abc_options;
mod hvigor_details;
pub mod snapshot;
pub mod system_capabilities;

pub use es2abc_options::SdkEs2abcOptionSnapshot;
pub use hvigor_details::{
    HvigorSdkComponentMetadata, HvigorSdkDetailsError, HvigorSdkDetailsInventory,
    HvigorSdkDetailsRequest, HvigorSdkDetailsSnapshot,
};
pub use snapshot::{SdkCrossplatformDependency, SdkSnapshot};
pub use system_capabilities::{
    SdkSystemCapabilityError, SdkSystemCapabilityInventory, SdkSystemCapabilitySelection,
};
