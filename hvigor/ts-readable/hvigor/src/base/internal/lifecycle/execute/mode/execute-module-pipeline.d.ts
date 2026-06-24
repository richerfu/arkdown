/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/execute-module-pipeline.d.ts
 */
import { Project } from '../../../../external/core/hvigor-core-node';
import { AbstractExecutePipeline } from './abstract-execute-pipeline';
import { TaskExecuteOptions } from './execute-mode-factory';
/**
 * 执行所有Module的Pipeline
 *
 * @since 2022/6/20
 */
export declare class ExecuteModulePipeline extends AbstractExecutePipeline {
    private executeModules;
    constructor(project: Project, taskOptions: TaskExecuteOptions);
    /**
     * 根据命令行和工程的配置，初始化-m 为module下时需要执行的所有模块集合
     *
     * @private
     */
    private init;
    findPipeAllNeedExecuteSyncTasks(): string[];
    findPipeAllNeedExecuteTasks(): string[];
    recordTraceData(): void;
}
