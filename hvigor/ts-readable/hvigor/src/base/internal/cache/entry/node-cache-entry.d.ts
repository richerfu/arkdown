/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/node-cache-entry.d.ts
 */
import { TaskSnapShotCacheEntry } from './tasksnapshot-cache-entry';
import { TreeCacheEntry } from './tree-cache-entry';
/**
 * 包含多个TaskCacheEntry,对应一个HvigorNode的结构的对象
 *
 * @since 2022/9/1
 */
export declare class NodeCacheEntry extends TreeCacheEntry<undefined, TaskSnapShotCacheEntry> {
    constructor(key: string, content?: undefined);
}
