/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-profile.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
/**
 * 处理module.json
 * 1. 写入ark字段
 * 2. 写入compileMode字段
 * 3. 替换target相关逻辑
 *  3.1 读取target的deviceType, 替换到module.json中
 *
 * @since 2022/1/10
 */
export declare class ProcessProfile extends OhosHapTask {
    private readonly _log;
    private readonly _arkEnable;
    private readonly _deviceTypes;
    private readonly _intermediatesMergeProfile;
    private readonly _processedModuleJson;
    private _moduleTargetData;
    private _pathInfo;
    private _dependencies;
    private harUIAbilities;
    private harExtensionAbilities;
    private readonly fakeUIAbilityPath;
    private duplicateAbilities;
    private hspNeedDeduplicateHarRootPathSet;
    private allHspDepTargets;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    private initHarDeduplicateData;
    private shouldCollectDependenciesAbilities;
    private get allowToAddFakeUIAbility();
    private get isFakeUIAbilityExists();
    /**
     * 预览场景下增加FakeUIAbility
     * @private
     */
    private addFakeUIAbility;
    beforeAlwaysAction(): Promise<void>;
    private validateExtensionAbilities;
    protected doTaskAction(): void;
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    /**
     * 获取依赖hsp的moduleName和versionCode，应用间hsp还会额外获取bundleName
     * @return {DependencyObj[]} 返回一个包含moduleName和versionCode的对象数组
     */
    private mergeHspLibs;
    /**
     * 搜集依赖har中的UIAbility和ExtensionAbility
     * @return ABILITY_RESULT
     */
    private collectHarAbilities;
    private addHarAbilitiesToModuleJson;
    /**
     * 构建hap、hsp时支持将直接、间接依赖har的module.json的abilities以及extensionAbilities合并到最终的hap、hsp产物module.json中，合并abilities时如果有name相同的则进行报错
     * @private
     */
    private processDependencyHarAbility;
    private findDuplicatedAbilityNames;
    /**
     * 检查依赖har中abilities数组的ability对象是否存在重复name
     * @private
     * @param moduleAndDepAbilities
     */
    private checkDuplicateAbility;
    /**
     * 添加模块依赖项
     * 对于模块HAR并且HAR_EXCLUDE_HSP_DEPENDENCIES参数为true，则不为模块添加dependencies
     * @param mergedModuleOpt 合并后的模块选项对象
     */
    private addModuleDependencies;
    /**
     * 如果是formExtensionModule，对应的资源不在hap中进行处理，过滤掉moduleOpt信息中form的extensionAbilities
     *
     * @private
     * @param mergedModuleOpt 合并后的模块选项对象
     */
    private processFormExtensionAbilities;
    /**
     * 是否需要排除去重har包里的abilities
     * @param dependency 依赖
     * @private
     */
    private shouldExcludeDuplicateHarDependency;
    /**
     * 是否有搜集到需求去重的har包ability
     * @private
     */
    private hasDuplicatedHarAbilities;
    /**
     * 处理har包去重逻辑
     * @private
     */
    private processHarDeduplicate;
    /**
     * 编译hap时合并依赖的所有hsp搜集到去重har中的abilities
     * @param moduleOptObj module obj
     * @private
     */
    private addDuplicateAbilities;
    private mergeDuplicateAbilities;
    /**
     * 把hsp搜集到har中的abilities写入到中间临时文件
     * @private
     */
    private writeDuplicateHarAbilitiesToIntermediateFile;
    /**
     * 编译hap时把hsp搜集到的去重ability中间文件作为输入
     * @param inputFiles 输入文件
     */
    private processHarDeduplicateInputFiles;
}
