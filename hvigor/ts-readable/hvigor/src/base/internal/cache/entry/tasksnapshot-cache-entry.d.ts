/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/tasksnapshot-cache-entry.d.ts
 */
import { TaskSnapshot } from '../../snapshot/core/task-snapshot';
import { CacheEntry } from './cache-entry';
/**
 * 包装TaskSnapShot的实例
 *
 * @since 2022/9/1
 */
export declare class TaskSnapShotCacheEntry extends CacheEntry<TaskSnapshot> {
    constructor(key: string, taskSnapShot: TaskSnapshot);
}
