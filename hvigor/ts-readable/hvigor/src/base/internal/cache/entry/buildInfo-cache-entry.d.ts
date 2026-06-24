/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/buildInfo-cache-entry.d.ts
 */
import { CacheEntry } from './cache-entry';
import { Snapshot } from '../../snapshot/core/snapshot';
/**
 * 包装构建信息SnapShot的实例
 *
 */
export declare class BuildInfoCacheEntry extends CacheEntry<Snapshot> {
    constructor(key: string, buildInfoSnapShot: Snapshot);
}
