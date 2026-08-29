//! Rust-owned compatibility surface for the Hvigor configuration runtime.
//!
//! JavaScript retains only values that must stay in the owning V8 isolate.
//! Ordering, lifecycle scheduling, and value-independent utility behavior live
//! here so replacing the DevEco-only `@ohos/hvigor` package can proceed one
//! behavior owner at a time.

mod cache_store;
mod constants;
mod global_data;
mod io;
mod lifecycle;
mod parameter;
mod task;
mod worker;

pub use cache_store::RuntimeHvigorCacheStoreManager;
pub use constants::RuntimeHvigorConstantApi;
pub use global_data::RuntimeHvigorGlobalData;
pub use io::RuntimeHvigorFileApi;
pub use lifecycle::RuntimeHvigorLifecycle;
pub use parameter::RuntimeHvigorParameterState;
pub use task::{
  RuntimeHvigorFileSet, RuntimeHvigorTaskExecutionState, RuntimeHvigorTaskHooks,
  RuntimeHvigorTaskState,
};
pub use worker::RuntimeHvigorWorkerPoolState;
