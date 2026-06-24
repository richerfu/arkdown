/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sync/sync-module.d.ts
 */
import { DefaultSyncTask, Module, TaskDetails } from '@ohos/hvigor';
/**
 * Module级别的Sync任务
 *
 * @since 2022/2/19
 */
export declare class SyncModule extends DefaultSyncTask {
    private readonly isFaMode;
    constructor(module: Module, taskName: string | TaskDetails, isFaMode: boolean);
    registryAction: () => Function;
}
