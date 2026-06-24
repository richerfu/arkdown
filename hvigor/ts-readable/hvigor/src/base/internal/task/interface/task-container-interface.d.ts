/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/interface/task-container-interface.d.ts
 */
import { CoreTask } from '../../../external/task/core-task';
import { TaskCreation } from '../core/lazy-task-container';
/**
 * 定义Hvigor任务管理容器支持的方法，主要包括添加，删除，查找等
 *
 * @since 2022/4/24
 */
export interface TaskContainer {
    /**
     * 往容器中增加Task
     *
     * @param {Task} task
     */
    addTask(task: CoreTask): void;
    /**
     * 注册任务TaskCreation
     *
     * @param action
     */
    registerTask(action: TaskCreation): void;
    /**
     * 从容器中删除Task
     *
     * @param {string} taskName
     * @return {boolean}
     */
    deleteTask(taskName: string): boolean;
    /**
     * 从容器中查找Task
     *
     * @param {string} taskName
     * @returns {Task | undefined}
     */
    findTask(taskName: string): CoreTask | undefined;
    /**
     * 从容器中查找任务依赖
     *
     * @param {string} taskPath
     * @returns {string[]}
     */
    getTaskDepends(taskPath: string): string[];
    /**
     * 判断是否存在该task
     *
     * @param {string} taskName
     * @returns {boolean}
     */
    hasTask(taskName: string): boolean;
    /**
     * 获取容器中的所有Task
     *
     * @returns {Task[]}
     */
    getAllTasks(): CoreTask[];
    getTaskPaths(): string[];
    /**
     * 清理容器中所有Task
     */
    clearTasks(): void;
}
