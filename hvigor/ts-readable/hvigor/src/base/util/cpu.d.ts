/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/cpu.d.ts
 */
export declare let cpuUsage: number;
/**
 * 开始监测CPU使用率
 */
export declare function startMonitor(): void;
/**
 * 停止监测CPU使用率
 */
export declare function endMonitor(): Promise<void>;
