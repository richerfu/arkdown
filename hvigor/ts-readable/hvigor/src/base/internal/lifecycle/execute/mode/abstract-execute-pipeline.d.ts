/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/abstract-execute-pipeline.d.ts
 */
import { HvigorCoreNode, Project } from '../../../../external/core/hvigor-core-node';
import type { TaskExecuteOptions } from './execute-mode-factory';
/**
 * 根据不用场景创建的pileLine的抽象公共类
 *
 * @since 2022/6/20
 */
export declare abstract class AbstractExecutePipeline {
    protected project: Project;
    protected taskOptions: TaskExecuteOptions;
    protected shouldCheckTask: boolean;
    protected toRunTask: string[];
    protected constructor(project: Project, taskOptions: TaskExecuteOptions);
    /**
     * 开始执行该pipeline
     */
    startPipeline(): Promise<void>;
    /**
     * 返回各模块所需执行的任务path
     *
     * @param nodes
     * @protected
     */
    protected findAllNeedExecuteTasksForNodes(nodes: HvigorCoreNode[]): string[];
    /**
     * 初始化一个Node的打点数据
     */
    recordTraceData(): void;
    /**
     * 执行一个Mode的Task
     */
    private executeTask;
    abstract findPipeAllNeedExecuteSyncTasks(): string[];
    abstract findPipeAllNeedExecuteTasks(): string[];
}
