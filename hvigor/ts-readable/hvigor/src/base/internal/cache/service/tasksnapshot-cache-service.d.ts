/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/tasksnapshot-cache-service.d.ts
 */
import { TaskSnapshot } from '../../snapshot/core/task-snapshot';
import { TaskSnapShotCacheEntry } from '../entry/tasksnapshot-cache-entry';
import { DefaultCacheService } from './default-cache-service';
/**
 * 提供获取任务缓存快照的统一服务接口
 *
 * @since 2022/9/1
 */
export declare class TaskSnapShotCacheService extends DefaultCacheService<TaskSnapshot> {
    private readonly _taskSnapShotCachePath;
    constructor(taskSnapShotCachePath: string);
    set(key: string, entryContent: TaskSnapshot): TaskSnapShotCacheEntry;
    initialize(): void;
}
