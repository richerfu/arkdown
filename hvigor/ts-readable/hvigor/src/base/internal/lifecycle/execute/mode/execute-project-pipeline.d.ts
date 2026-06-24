/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/execute-project-pipeline.d.ts
 */
import { Project } from '../../../../external/core/hvigor-core-node';
import { AbstractExecutePipeline } from './abstract-execute-pipeline';
import { TaskExecuteOptions } from './execute-mode-factory';
/**
 * 执行工程Project的Pipeline
 *
 * @since 2022/6/20
 */
export declare class ExecuteProjectPipeline extends AbstractExecutePipeline {
    constructor(project: Project, taskOptions: TaskExecuteOptions, shouldCheckTask: boolean);
    findPipeAllNeedExecuteSyncTasks(): string[];
    findPipeAllNeedExecuteTasks(): string[];
}
