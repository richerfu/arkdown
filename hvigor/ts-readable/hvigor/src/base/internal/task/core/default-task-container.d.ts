/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/default-task-container.d.ts
 */
import { CoreTask } from '../../../external/task/core-task';
import { TaskContainer } from '../interface/task-container-interface';
import { TaskCreation } from './lazy-task-container';
import { TaskDirectedAcyclicGraph } from './task-directed-acyclic-graph';
/**
 * module的Task容器管理的默认实现
 *
 * @since 2022/4/24
 */
export declare class DefaultTaskContainer implements TaskContainer {
    private readonly _moduleName;
    private _tasks;
    private _taskGraph;
    private _log;
    constructor(moduleName: string, taskGraph: TaskDirectedAcyclicGraph);
    registryTask(creation: TaskCreation): void;
    getTaskPaths(): string[];
    registerTask(action: TaskCreation): void;
    getTaskDepends(taskPath: string): string[];
    addTask(task: CoreTask): void;
    deleteTask(taskName: string): boolean;
    findTask(taskName: string): CoreTask | undefined;
    getAllTasks(): CoreTask[];
    hasTask(taskName: string): boolean;
    clearTasks(): void;
}
