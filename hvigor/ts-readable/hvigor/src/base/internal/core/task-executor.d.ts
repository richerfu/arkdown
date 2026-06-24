/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/core/task-executor.d.ts
 */
export declare class TaskExecutor {
    /**
     * 通过指定task的path，执行对应的任务
     *
     * @param {string[]} taskPaths 需要执行的所有任务路径
     */
    initTasks(taskPaths: string[]): Promise<void>;
}
export declare const taskExecutorInst: TaskExecutor;
