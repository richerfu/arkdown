/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/default-sync-task.d.ts
 */
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
import { DefaultTask } from '../../../external/task/default-task';
import { TaskDetails } from '../interface/task-details-interface';
/**
 * Hvigor同步类型任务的基础类,同步类型的任务是用来在插件嵌入到其他平台中提供数据同步作用的
 * 为了跟执行构建任务区分，独立出来，单独扩展和使用，同时可以提高执行构建任务的效率，免去不必要的数据同步
 *
 * @since 2022/1/20
 */
export declare abstract class DefaultSyncTask extends DefaultTask {
    protected constructor(defaultModule: HvigorCoreNode, taskName: string | TaskDetails);
}
