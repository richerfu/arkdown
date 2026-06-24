/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sync/sync-project.d.ts
 */
import { DefaultSyncTask, Project, TaskDetails } from '@ohos/hvigor';
/**
 * Project级别的Sync任务
 *
 * @since 2022/2/19
 */
export declare class SyncProject extends DefaultSyncTask {
    constructor(project: Project, taskName: string | TaskDetails);
    registryAction: () => Function;
}
