/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/project-cache-entry.d.ts
 */
import { NodeCacheEntry } from './node-cache-entry';
import { TreeCacheEntry } from './tree-cache-entry';
/**
 * 包含多个NodeCacheEntry,对应一个Hvigor工程结构的对象
 *
 * @since 2022/9/1
 */
export declare class ProjectCacheEntry extends TreeCacheEntry<NodeCacheEntry, NodeCacheEntry> {
    constructor(key: string, content: NodeCacheEntry);
}
