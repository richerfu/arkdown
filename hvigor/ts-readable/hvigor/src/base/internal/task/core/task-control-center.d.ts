/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-control-center.d.ts
 */
/// <reference types="node" />
/// <reference types="node" />
import EventEmitter from 'events';
import { CoreTask } from '../../../external/task/core-task';
/**
 * 用于管理任务执行状态
 *
 * @since 2022/08/13
 */
export declare class TaskControlCenter extends EventEmitter {
    private readonly _log;
    private readonly blockedTaskMap;
    private readonly runnableTaskQueue;
    private readonly runningTaskList;
    private intervalId;
    constructor();
    clear(): void;
    initTaskQueueBeforeRun(executeTaskPaths: string[]): void;
    private initTaskQueueForOneTaskNew;
    /**
     * 更新需要进行类型检查的构建脚本集合
     *
     * @param task 需要执行的任务
     */
    private updateTargetRoots;
    clearRunnableTaskQueue(): void;
    clearBlockedTaskMap(): void;
    allTasksHasExecuted(): boolean;
    popRunnableTask(): string | undefined;
    getRunnableTaskSize(): number;
    setTaskExecuted(executedTaskPath: string): void;
    pushRunningTask(task: CoreTask): void;
    getRunningTask(taskPath: string): CoreTask | undefined;
    setIntervalId(intervalId: NodeJS.Timeout | undefined): void;
    clearInterval(): void;
    popPriorTask(): string | undefined;
}
export declare const taskControlCenter: TaskControlCenter;
