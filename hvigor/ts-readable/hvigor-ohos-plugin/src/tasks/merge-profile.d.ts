/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/merge-profile.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { AbstractMergeProfile } from './abstract-merge-profile';
import { TargetTaskService } from './service/target-task-service';
/**
 * 合并app.json和module.json5,合并依赖的har中的module.json5
 *
 * @since 2022/1/10
 */
export declare class MergeProfile extends AbstractMergeProfile {
    private _appRes;
    private _moduleTargetRes;
    private readonly _isHarModule;
    private readonly _multiProjects;
    private _ohLibs;
    private readonly _mergedModuleJson;
    private readonly _moduleBuildAbilities;
    constructor(taskService: TargetTaskService);
    declareInputs(): Map<string, TaskInputValue>;
    private get includeAppScopeRes();
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    get isBundledDependencies(): boolean;
    beforeAlwaysAction(): Promise<void>;
    initTaskDepends(): void;
    protected initHarModuleDepends(): void;
    protected doTaskAction(): Promise<void>;
    /**
     * 将上次编译的中间产物的module.json5与本次处理新生成的mergeModuleOpt对象进行对比
     * 若存在部分场景不一致,需要在执行ArkTs编译前删除历史缓存:
     * 1. 外部模块且包名不一样
     * 2. sdk版本变更
     *
     * @param {AppJson.AppOptObj & ModuleJson.ModuleOptObj} mergedModuleOpt
     * @returns {Promise<void>}
     * @private
     */
    private clearArkTSCacheByModuleJSONChange;
    private computeAppJsonOpt;
    private mergeAppEnvironmentConfig;
    private processResourceStr;
    private generateResourceStrByPermissions;
    private copyStringToResourceStr;
    /**
     * 过滤资源字符串文件，根据给定的资源字符串数组，保留匹配的字符串项。
     * 如果文件夹名称不是 'rawfile' 或 'resfile'，则处理该文件夹下的字符串资源。
     * 如果处理后的文件没有匹配的字符串项，则删除该文件夹。
     *
     * @param resStrPath - 资源字符串路径，包含多个文件夹，每个文件夹代表一个资源文件。
     * @param resourceStr - 需要保留的资源字符串名称数组。
     */
    private filterResStr;
    private mergeDslConfig;
    private resolveApi;
    /**
     * 获取所有需要移除的权限的名称集合，并提供重复告警
     * @param moduleOpt module对象
     * @return 返回需要移除的权限的名称集合
     */
    private getRemovePermissionSet;
    /**
     * 获取依赖三方包中module文件转换得到的json对象列表，依赖包括本地或远程hsp、har，合并其中module文件json对象的部分内容
     * @param moduleOpt 本模块module文件的json对象
     * @private
     */
    private mergeAllHarModuleOpt;
    /**
     * bundledDependencies为true时需要合并其依赖har的module.json5中的metadata和requestPermission
     * @param moduleOpt
     * @private
     */
    private mergeBundledDependenciesHar;
    /**
     * metadata与requestPermissions需要去重
     * 1.参考hap/hsp规则.若字节码har依赖源码har出现重复name对象:保留字节码har的配置,去除依赖源码har的重复项
     * 2.若依赖源码har之间出现重复name对象:仅收集初次遍历得到的配置项
     * @param depModuleConfigArr 依赖har的metadata或requestPermissions对象
     * @param moduleConfigArr 字节码har的metadata或requestPermissions对象
     * @private
     */
    private deDuplicateArr;
    /**
     * 合并三方依赖库， 包括本地依赖和远程依赖
     * @param jsonType 用于指定json文件类型，比如config或module
     * @return 返回一个字符串数组，包含了合并后的三方依赖库路径
     */
    protected mergeDependsLibs(jsonType: string): string[];
    /**
     * 将三方依赖的module文件转换为ModuleOpt对象数组，不止har依赖，还包括hsp
     * @return {HarModuleOpt[]} 返回ModuleOpt对象数组，不止har依赖，还包括hsp，不要被命名误导
     */
    private transformJson2Opts;
    /**
     * 将依赖的三方包json对象的requestPermissions合并到tempMergedRequestPermission中
     * @param tempMergedRequestPermission 已合并的临时RequestPermission列表
     * @param harModuleOpt 依赖的三方包json对象
     * @private
     */
    private mergeHarRequestPermission;
    private mergeArrayOptions;
    private checkHarApiStatus;
    private harMinApiVersionToAPi;
    /**
     * 定制处理product中部分字段配置
     * @param mergedModuleOpt
     * @param curProduct
     * @private
     */
    private customizeModuleOpt;
    private getMergedAppOptObj;
    private mergeBuildProfileAbilities;
    /**
     * 初始化资源字符串。
     *
     * 该方法处理HSP和HAR依赖项的资源字符串，将依赖项中的字符串合并到模块的字符串文件中。
     * 如果字符串文件不存在或格式不正确，将抛出错误。
     */
    protected initResourceStr(): void;
    /**
     * 检查模块文件的JSON格式是否正确。
     *
     * @param fileJson - 要检查的文件JSON对象。
     * @param filePath - 要检查的文件的路径，用于错误信息输出。
     */
    private checkJsonFormat;
}
