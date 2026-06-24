/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/proxy/task-proxy.d.ts
 */
import { CoreTaskImpl } from '../../../external/task/core-task-impl';
import { BasicFileSnapshot } from '../../snapshot/core/filesystem-snapshot';
import { TaskSnapshot } from '../../snapshot/core/task-snapshot';
import { FsOptions } from '../../snapshot/util/file-set';
/**
 * 增量Task的一个中间代理类，用于在任务实际执行前进行一些增量检查,执行后,刷新增量缓存之类的
 *
 * @since 2022/9/1
 */
export declare class TaskProxy {
    private readonly project;
    private readonly task;
    private readonly taskNode;
    private readonly cacheTaskSnapShot;
    private projectCacheService;
    private snapshotGeneratorService;
    private snapshotComparatorService;
    private curTaskSnapShot;
    private curInputFileSnapShotMap;
    private readonly notDeclareAnyIncrementalLogic;
    private readonly useInputOutputCache;
    constructor(task: CoreTaskImpl);
    /**
     * 判断单个文件是否发生变化
     *
     * @param {string} filePath
     * @param options
     * @return {[BasicFileSnapshot, boolean]}
     * @private
     */
    getFileCompareResult(filePath: string, options?: FsOptions): [BasicFileSnapshot, boolean];
    /**
     * 检查任务快照中具体key对应值与上次构建相比是否发生变化
     *
     * @param {string} filePath
     * @param options
     * @return {[BasicFileSnapshot, boolean]}
     * @private
     */
    taskInputValueChanged(InputKey: string): boolean;
    /**
     * 判断单个Task是否发生变化
     *
     * @return {boolean}
     * @private
     */
    private isTaskChange;
    /**
     * 判断任务设置的增量输入文件是否发生变化
     *
     * @return {boolean}
     * @private
     */
    private isTaskInputFilesChange;
    /**
     * 判断任务设置的增量输出文件是否发生变化
     *
     * @return {boolean}
     * @private
     */
    private isTaskOutputFilesChange;
    /**
     * 执行任务之前的前置任务，比如增量的检查
     *
     * @return {boolean} true/false true->不需要重新执行,false->需要重新执行
     */
    preExecute(): boolean;
    /**
     * 执行任务的实际TaskAction逻辑
     */
    execute(): Promise<void>;
    /**
     * 任务执行成功后的刷新缓存
     */
    postExecute(): void;
    private updateTaskOutputFilesSnapShot;
    private updateTaskInputFilesSnapshot;
    private updateTaskSnapShot;
    getCurTaskSnapShot(): TaskSnapshot | undefined;
}
