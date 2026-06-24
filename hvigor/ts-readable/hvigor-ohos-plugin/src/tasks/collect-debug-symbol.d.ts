/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/collect-debug-symbol.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { OhosLogger } from '../utils/log/ohos-logger';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
/**
 * 将打hap/hsp/hsp/app等构建过程中的debug symbol收集到产物指定目录symbol下
 */
export declare class CollectDebugSymbol extends OhosHapTask {
    protected _log: OhosLogger;
    private readonly _taskService;
    private readonly RELEASE;
    private readonly DEBUG;
    private readonly debugSymbolPath;
    private readonly modulePath;
    private isRelease;
    private shouldCollectDebugSymbol;
    constructor(taskService: TargetTaskService);
    /**
     * 检查增量输入输出
     * 输入检查所有要收集的文件的原路径
     * 输出检查整个输出文件夹
     */
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    /**
    检查配置是否发生改变
     */
    beforeAlwaysAction(): Promise<void>;
    /**
    根据shouldCollectDebugSymbol来判断是否应该执行收集任务
     */
    taskShouldDo(): boolean;
    protected doTaskAction(): void;
    private copyNameCache;
    private copySourceMap;
    initTaskDepends(): void;
    /**
     * 返回输出目标路径，不存在就创建，保证输出不为空，否则会影响之后创建软连接
     * @private
     * @return destinationPath
     */
    private getDestinationPath;
    /**
     * 建立so文件夹的软连接到目标文件夹,软连接的使用方法要考虑跨平台的兼容性
     * @private
     */
    private createSymbolicLinkForSoFolder;
    private getModuleName;
}
