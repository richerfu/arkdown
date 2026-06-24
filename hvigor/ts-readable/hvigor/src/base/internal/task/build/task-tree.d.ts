/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/build/task-tree.d.ts
 */
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
import { DefaultTask } from '../../../external/task/default-task';
/**
 * 输出打印指定Node的task DAG图结构
 *
 * @since 2022/5/31
 */
export declare class TaskTree extends DefaultTask {
    constructor(hvigorNode: HvigorCoreNode);
    registryAction(): Function;
    private renderTaskGraph;
    private getEndNodePrefix;
    private getExpandNodePrefix;
}
