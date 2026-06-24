/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/build/tasks.d.ts
 */
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
import { DefaultTask } from '../../../external/task/default-task';
/**
 * 通过执行hvigor 命令打印Hvigor Node的任务详细信息
 * eg.hvigor tasks
 *
 * @since 2022/5/31
 */
export declare class Tasks extends DefaultTask {
    constructor(hvigorNode: HvigorCoreNode);
    registryAction(): Function;
    private transformInfos;
}
