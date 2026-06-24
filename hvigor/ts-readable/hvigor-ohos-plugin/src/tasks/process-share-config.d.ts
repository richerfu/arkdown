/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-share-config.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
export declare class ProcessShareConfig extends OhosHapTask {
    private readonly _log;
    private readonly shareConfigFileName;
    private readonly intermediatesTempShareConfigFilePath;
    constructor(targetService: TargetTaskService);
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    declareInputs(): Map<string, TaskInputValue>;
    beforeAlwaysAction(): Promise<void>;
    protected doTaskAction(): void;
    /**
     * 处理har模块的shareConfig
     * @param harShareConfigObjList
     * @private
     */
    private processHarShareConfig;
    /**
     * 处理entry/feature/hsp模块的shareConfig
     * @param hapOrHspShareConfigObjList
     * @private
     */
    private processHapOrHspShareConfig;
    /**
     * 处理模块自有shareConfig
     * @param allShareConfigObjList
     * @param moduleSelfShareConfigObjList
     * @private
     */
    private processModuleSelfShareConfig;
    /**
     * 处理模块依赖的shareConfig
     * @param allShareConfigObjList
     * @private
     */
    private processModuleDependencyShareConfig;
    private getLocalDependencyShareConfig;
    /**
     * 处理模块本地依赖的shareConfig
     * @param localDependency 模块直接依赖/间接依赖
     * @param allShareConfigObjList
     * @param moduleFlag2ShareConfigObjList
     * @private
     */
    private processLocalDependencyShareConfig;
    /**
     * 处理模块远程依赖的shareConfig
     * @param remoteDependency
     * @param allShareConfigObjList
     * @private
     */
    private processRemoteDependencyShareConfig;
    /**
     * bundleHAR依赖的源码har在temp_share_config.json中新加入一个packageName字段标识属于哪个模块
     * @param dependencyShareConfigObj
     * @param dependency
     * @private
     */
    private shareConfigAddPackageName;
    /**
     * 校验shareConfig集合中对象uri唯一
     * @param shareConfigObjList
     * @private
     */
    private checkShareConfigObjUri;
    initTaskDepends(): void;
    /**
     * 将处理过后的shareConfig配置集合保存在中间产物临时目录中
     * @param shareConfigObjList
     * @private
     */
    private generateTempshareConfig;
}
