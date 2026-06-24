/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/task/impl/transform-hvigor-task.d.ts
 */
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
import { IncrementalTask } from '../../../external/task/incremental-task';
import { FileSet } from '../../../internal/snapshot/util/file-set';
import { TaskInputValue } from '../../../internal/snapshot/util/task-input-value-entry';
import { HvigorTask, HvigorTaskContext } from '../interface/hvigor-task';
/**
 * hvigorTask转换为CoreTask
 *
 * @since 2023/12/20
 */
export declare class TransformHvigorTask extends IncrementalTask {
    private logger;
    private _inputs;
    private _inputFiles;
    private _outputFiles;
    protected task: HvigorTask;
    protected taskContext: any;
    protected taskRunContext: HvigorTaskContext;
    constructor(node: HvigorCoreNode, task: HvigorTask);
    initTaskContext(): void;
    /**
     * 初始化任务依赖关系
     * 支持两种格式的依赖初始化方式
     * 1. node:target@task 或 node:task
     * 2. target@task 或 task
     * @return {void}
     */
    initDependency(): void;
    /** 通过设置postDependency，将自定义任务挂上任务执行流程
     * 支持两种格式的依赖初始化方式
     * 1. node:target@task 或 node:task
     * 2. target@task 或 task
     * @param postDependency 后置依赖
     * @param taskName 前置依赖
     */
    initPostDependencyTask(postDependency: string, taskName: string): void;
    initTaskRun(): void;
    getAction(): Function;
    doTaskAction(): Promise<void>;
    getTaskParam(): HvigorTaskContext;
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
}
