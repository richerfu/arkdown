/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/lazy-task-container.d.ts
 */
import { CoreTask } from '../../../external/task/core-task';
import { TaskContainer } from '../interface/task-container-interface';
import { TaskDetails } from '../interface/task-details-interface';
export type TaskCreation = {
    provider: () => CoreTask;
    depends: string[];
    details: TaskDetails;
    dependsDefinedByUser: string[];
};
export declare class LazyTaskContainer implements TaskContainer {
    private readonly _moduleName;
    private _tasks;
    private _lazyTasks;
    constructor(moduleName: string);
    registerTask(creation: TaskCreation): void;
    addTask(task: CoreTask): void;
    deleteTask(taskName: string): boolean;
    hasTask(taskName: string): boolean;
    getLazyTask(task: string): TaskCreation | undefined;
    getTask(task: string): CoreTask | undefined;
    /**
     * 获取所有任务
     * 仅返回已初始化的task
     */
    getAllTasks(): CoreTask[];
    getAllTasksCA(): TaskCreation[];
    getTaskPaths(): string[];
    findTask(taskName: string): CoreTask | undefined;
    findTaskCA(taskName: string): TaskCreation | undefined;
    getTaskDepends(taskPath: string): string[];
    clearTasks(): void;
    private initializeLazyTask;
    private mergeDepends;
}
