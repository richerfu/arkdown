/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/compile-resource.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { AbstractCompileResource } from './abstract/abstract-compile-resource';
import { ModuleTargetData } from './data/hap-task-target-data';
import { TargetTaskService } from './service/target-task-service';
/**
 * 编译资源
 *
 * @since 2022/1/10
 */
export declare class CompileResource extends AbstractCompileResource {
    private _log;
    protected processProfileTask: string;
    private resConfigFilePathForHarDeduplicate;
    beforeAlwaysAction(): Promise<void>;
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    declareExecutionCommand(): string;
    constructor(targetTaskService: TargetTaskService);
    initTaskDepends(): void;
    protected doTaskAction(): Promise<void>;
    invokeRestool(targetData: ModuleTargetData): Promise<void>;
    private getRestoolEnv;
    /**
     * hsp去重时用har资源路径去重后的 resConfig.json 再调用一次 restool 来生成去重后的resources和resources.index共打包使用
     */
    private compileResourcesForHarDeduplicate;
    /**
     * 处理stage模型target定制distroFilter场景
     * target中定制distroFilter配置写入metadata->{resource文件}下
     * @protected
     */
    protected customizeDistroFilter(): void;
    /**
     * 处理shortcuts字段bundleName定制场景
     * @constructor
     * @protected
     */
    protected customizeShortcut(): void;
    private processShortcutObjBundleName;
    /**
     * 将中间产物资源目录下的shareConfig配置修改为合并后的shareConfig配置
     * @param shareConfigObjList
     * @private
     */
    private writeShareConfig;
    private processShareConfig;
    /**
     * 将中间产物资源目录下的routerMap配置修改为合并后的routerMap配置
     * @param routerMapObjList
     * @private
     */
    private writeRouterMap;
    private processRouterMap;
    /**
     * 处理启动框架相关任务，从intermediate/startup下读取合并后的文件，处理后写入到intermediates/res下的资源文件中，如有必要还要修改res下的module.json
     *
     * @private
     */
    private processStartupConfig;
    private checkStartupTaskDependencies;
    private validateDependencies;
    /**
     * 从intermediates/startup中读取合并后的启动框架配置，循环遍历拿到所有startupTask和appPreloadHintStartupTask的name属性集合
     *
     * @param startupMergedOptions
     * @private
     */
    private getAllStartupTaskNames;
    /**
     * 是否需要为har包去重二次编译
     * @private
     */
    private shouldCompileResourceForHarDeduplicate;
}
