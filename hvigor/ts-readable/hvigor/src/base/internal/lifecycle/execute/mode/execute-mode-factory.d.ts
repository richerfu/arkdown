/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/execute-mode-factory.d.ts
 */
import { Project } from '../../../../external/core/hvigor-core-node';
import { AbstractExecutePipeline } from './abstract-execute-pipeline';
/**
 * CoreTask 执行配置
 *
 * @since 2022/5/18
 */
export interface TaskExecuteOptions {
    toRunTasks: string[];
    isNeedSync: boolean;
}
/**
 * 一个简单工厂，用于根据命令行中用户指定的模式，创建出不同的pipeline管道
 *
 * @since 2022/6/18
 */
export declare class ExecuteModeFactory {
    private readonly project;
    private readonly taskOptions;
    constructor(project: Project, taskOptions: TaskExecuteOptions);
    /**
     * 根据用户指定的不同模式创建不同的执行管道
     * -m project ->ExecuteProjectPipeline
     * -m module ->ExecuteModulePipeline
     * 不指定 ->ExecuteUndefinedPipeline
     *
     * @param {string} commandMode
     * @returns {AbstractExecutePipeline}
     */
    getExecutePipeline(commandMode: string): AbstractExecutePipeline;
}
