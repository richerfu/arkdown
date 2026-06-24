/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/match-task-with-node.d.ts
 */
export type TaskWithNode = {
    node: string;
    taskName: string;
};
/**
 * 从完整的任务名中解析出节点名和任务名
 * @param {string} fullTaskName - 完整的任务名
 * @return {TaskWithNode | undefined} - 返回一个包含node和taskName的对象，如果解析失败则返回undefined
 */
export declare function matchTaskWithNode(fullTaskName: string): TaskWithNode | undefined;
