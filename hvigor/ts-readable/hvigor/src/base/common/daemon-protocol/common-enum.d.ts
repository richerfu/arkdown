/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/daemon-protocol/common-enum.d.ts
 */
export declare enum OutputType {
    SYSTEM = "system",
    STDOUT = "stdout",
    STDERR = "stderr"
}
export declare enum BuildStatus {
    FINISH = "finish",
    BEGIN = "begin",
    RUNNING = "running",
    CLOSE = "close",
    REJECT = "reject"
}
export declare enum LogLevel {
    DEBUG = "debug",
    INFO = "info",
    WARN = "warn",
    ERROR = "error"
}
