pub mod debounce;
pub mod deveco_ws;
pub mod event_name;
pub mod hot_reload;
pub mod invalidation;
pub(crate) mod layout;
pub mod watcher;

// Narrowed re-exports: only the types that form the public API surface.
pub use deveco_ws::{DevEcoEvent, DevEcoServerHandle, DevEcoWsServer, DEVECO_WS_PORT};
pub use invalidation::{
    classify_watch_change, InvalidationEvent, WatchChangeKind, WatchInvalidationBatch,
};
pub use watcher::{SessionWatcher, WatchEvent, WatchEventKind, WatchInput};

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("Filesystem watcher error: {0}")]
    Notify(#[from] notify::Error),
    #[error("Build failed: {0}")]
    Build(String),
}
