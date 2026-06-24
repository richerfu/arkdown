/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/execute-undefined-pipeline.d.ts
 */
import { Project } from '../../../../external/core/hvigor-core-node';
import { AbstractExecutePipeline } from './abstract-execute-pipeline';
import { TaskExecuteOptions } from './execute-mode-factory';
/**
 * 执行mode为undefined状态下的Pipeline
 *
 * @since 2022/6/20
 */
export declare class ExecuteUndefinedPipeline extends AbstractExecutePipeline {
    private projectPipeLine;
    private modulePipeline;
    constructor(project: Project, taskOptions: TaskExecuteOptions);
    findPipeAllNeedExecuteSyncTasks(): string[];
    findPipeAllNeedExecuteTasks(): string[];
}
