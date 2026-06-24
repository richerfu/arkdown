/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/daemon-protocol/build-event.js
 */
export var BuildEvent, WatchEvent;
(function (t) {
  t.COMMON_BUILD = "CommonBuild", t.CANCEL_BUILD = "CancelBuild", t.STOP_DAEMON = "StopDaemon", t.OUTPUT = "Output", t.BUILD_STATUS = "BuildStatus";
})(BuildEvent = BuildEvent || (BuildEvent = {})), function (t) {
  t.WATCH_START = "WatchStart", t.WATCH_RESULT = "WatchResult", t.CLOSE_WATCH = "CloseWatch", t.WATCH_COMPILE_RESULT = "WatchCompileResult", t.WATCH_COMPILE_DATA = "WatchCompileData", t.WATCH_LOG = "WatchLog", t.TERMINATE_WORKER = "TerminateWorker", t.NEW_WATCH_WORKER = "NewWatchWorker", t.SEND_KEEP_ALIVE_SIGNAL = "SendKeepAliveSignal";
}(WatchEvent = WatchEvent || (WatchEvent = {}));
