/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/daemon-protocol/build-event.d.ts
 */
export declare enum BuildEvent {
    COMMON_BUILD = "CommonBuild",
    CANCEL_BUILD = "CancelBuild",
    STOP_DAEMON = "StopDaemon",
    OUTPUT = "Output",
    BUILD_STATUS = "BuildStatus"
}
export declare enum WatchEvent {
    WATCH_START = "WatchStart",
    WATCH_RESULT = "WatchResult",
    CLOSE_WATCH = "CloseWatch",
    WATCH_COMPILE_RESULT = "WatchCompileResult",
    WATCH_COMPILE_DATA = "WatchCompileData",
    WATCH_LOG = "WatchLog",
    TERMINATE_WORKER = "TerminateWorker",
    NEW_WATCH_WORKER = "NewWatchWorker",
    SEND_KEEP_ALIVE_SIGNAL = "SendKeepAliveSignal"
}
