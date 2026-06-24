/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/filesnapshot-cache-entry.d.ts
 */
import { BasicFileSnapshot } from '../../snapshot/core/filesystem-snapshot';
import { CacheEntry } from './cache-entry';
/**
 * 包装文件缓存的实例CacheEntry
 *
 * @since 2022/9/1
 */
export declare class FileSnapShotCacheEntry extends CacheEntry<BasicFileSnapshot> {
    constructor(key: string, fileSnapShot: BasicFileSnapshot);
}
