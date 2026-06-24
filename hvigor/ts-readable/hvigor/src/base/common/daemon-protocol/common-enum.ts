/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/daemon-protocol/common-enum.js
 */
export var OutputType, BuildStatus, LogLevel;
(function (t) {
  t.SYSTEM = "system", t.STDOUT = "stdout", t.STDERR = "stderr";
})(OutputType = OutputType || (OutputType = {})), function (t) {
  t.FINISH = "finish", t.BEGIN = "begin", t.RUNNING = "running", t.CLOSE = "close", t.REJECT = "reject";
}(BuildStatus = BuildStatus || (BuildStatus = {})), function (t) {
  t.DEBUG = "debug", t.INFO = "info", t.WARN = "warn", t.ERROR = "error";
}(LogLevel = LogLevel || (LogLevel = {}));
