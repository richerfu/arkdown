/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/default-internal-task.d.ts
 */
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
import { CoreTaskImpl } from '../../../external/task/core-task-impl';
import { TaskDetails } from '../interface/task-details-interface';
/**
 * 通过Function直接注册的Task类型
 *
 * @since 2022/4/24
 */
export declare class DefaultInternalTask extends CoreTaskImpl {
    constructor(module: HvigorCoreNode, taskDetails: TaskDetails, fn: Function);
}
