/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/daemon-info.d.ts
 */
export interface DaemonInfo {
    keyId: string;
    hvigorVersion: string;
    cwdPath: string;
    cachePath: string;
    nodeVersion: string;
    nodeParams: string;
    address: string;
    port: number;
    pid: number;
    state: DaemonState;
    lastUsedTime: number;
    info?: string;
    createdBy: string;
    sessionId: string;
}
export declare enum DaemonState {
    IDLE = "idle",
    HALF_BUSY = "half_busy",
    BUSY = "busy",
    CANCELED = "canceled",
    STOP_REQ = "stopReq",
    STOPPED = "stopped",
    BROKEN = "broken"
}
