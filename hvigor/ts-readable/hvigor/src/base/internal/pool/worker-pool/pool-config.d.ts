/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/pool-config.d.ts
 */
export type PoolConfig = {
    maxPoolNum?: number;
    minPoolNum?: number;
    recycleInterval?: number;
    maxIdleTime?: number;
    maxCoreSize?: number;
    cacheCapacity?: number;
    cacheTtl?: number;
};
export declare function isSamePoolConfig(p1: PoolConfig, p2: PoolConfig): boolean;
