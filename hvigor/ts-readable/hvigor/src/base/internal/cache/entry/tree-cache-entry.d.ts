/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/tree-cache-entry.d.ts
 */
import { CacheEntry } from './cache-entry';
/**
 * 基于CacheEntry的基本结构封装的一个带有树状结构的容器类
 *
 * @since 2022/9/1
 */
export declare class TreeCacheEntry<E, T> extends CacheEntry<E> {
    private _subEntries;
    constructor(key: string, content: E);
    getSubEntry(key: string): T | undefined;
    setSubEntry(key: string, subEntry: T): void;
    getSunEntrySize(): number;
    remove(key: string): void;
}
