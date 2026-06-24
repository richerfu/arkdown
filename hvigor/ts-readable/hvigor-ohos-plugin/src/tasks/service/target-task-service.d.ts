/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/target-task-service.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { AotCompileModeEnum } from '../../enum/aot-compile-mode-enum';
import { ModuleModel } from '../../model/module/module-model';
import { BuildOpt, NativeLib } from '../../options/build/build-opt';
import { ModuleJson } from '../../options/configure/module-json-options';
import { RouterMapOptions } from '../../options/configure/router-map-options';
import { ShareConfigOptions } from '../../options/configure/share-config-options';
import { SdkInfo } from '../../sdk/sdk-info';
import { ModuleTargetData } from '../data/hap-task-target-data';
import { ModuleTaskService } from './module-task-service';
import ModuleOptObj = ModuleJson.ModuleOptObj;
import RouterMapObj = RouterMapOptions.RouterMapObj;
import ShareConfigObj = ShareConfigOptions.ShareConfigObj;
/**
 * 以Target为维度创建不同的task，提供target相关的数据信息和管理
 *
 * @since 2022/8/11
 */
export declare class TargetTaskService {
    private targetData;
    private buildOption;
    private readonly _sdkInfo;
    private readonly moduleService;
    private readonly shouldBuildCmake;
    constructor(targetData: ModuleTargetData, moduleService: ModuleTaskService);
    refreshBuildOption(): void;
    buildTaskName(taskDetails: TaskDetails, targetName?: string): TaskDetails;
    getTargetData(): ModuleTargetData;
    getBuildOption(): BuildOpt;
    isByteCodeHar(): boolean;
    isCustomizedHar(): boolean;
    /**
     * 获取byteCodeHar真实值
     * useNormalizedOHMUrl=true
     *    byteCodeHar若无配置则默认为true
     * useNormalizedOHMUrl=false
     *    byteCodeHar若无配置则默认为false
     * 非上述默认情况则以实际配置值为准
     *
     * @param useNormalizedOHMUrl 使用新格式ohmurl 默认为false 需显示配置才可生效
     * @param byteCodeHar
     */
    getByteCodeHar(useNormalizedOHMUrl: boolean | undefined, byteCodeHar: boolean | undefined): boolean;
    /**
     * 获取noExternalImportByPath真实值
     * useNormalizedOHMUrl=true时
     *    noExternalImportByPath若无配置则默认为true
     * 非上述默认情况则以实际配置值为准
     */
    isNoExternalImportByPath(): boolean;
    /**
     * 判断是否为源码har
     */
    isSourceCodeHar(): boolean;
    isBundledDependencies(): boolean;
    getNativeLibOption(): NativeLib;
    /**
     * 提供当前target绝对路径化的workerConfig
     * @returns {string[]}
     */
    getWorkerConfig(): string[];
    /**
     * 获取模块级build-profile.json5中所有自定义buildProfileFields的变量
     */
    getBuildProfileFields(): object | undefined;
    /**
     * 获取模块target资源目录集合
     */
    getResourceDirs(): string[];
    /**
     *  相对路径转换为资源绝对路径并校验
     *
     * @param {string}  resourcesDirs 资源路径
     */
    private convertResourceAbsolutePath;
    getAnBuildMode(): AotCompileModeEnum;
    getApPath(): string | undefined;
    getApAbsolutePath(): string;
    getCustomTypes(): string[] | undefined;
    isDebug(): boolean;
    /**
     * 判断是否为开源/闭源
     * 1.混淆配置仅release生效
     * 2.debug模式下均为开源
     * 3.release模式下检查arkOptions?.obfuscation?.ruleOptions?.enable字段
     *    3.1.若enable配置为false或缺省,则不开启混淆为开源
     *    3.2.若enable配置为true,则开启混淆为闭源
     */
    isOpenSource(): boolean;
    getShouldBuildCmake(): boolean;
    /**
     * 判断构建模式是debug还是release
     */
    getBuildMode(): string;
    needPackBin(target: ModuleTargetData): boolean;
    getSdkInfo(): SdkInfo;
    getModuleService(): ModuleTaskService;
    /**
     * 获取卡片配置文件路径的数组
     * e.g. ./src/main/resources/base/profile/form_config.json
     * 1.默认在当前模块的资源目录中索引卡片配置文件
     * 2.如果是独立卡片包，则会合并使用方的module.json的extensionAbilities信息用于后续校验
     *
     * @param taskService
     * @returns 卡片配置文件路径的数组
     */
    static getFormJsonArr(taskService: TargetTaskService): string[];
    /**
     * 获取卡片配置文件路径的数组
     *
     * @returns 卡片配置文件路径的数组
     * @param targetModuleOptObj
     * @param targetResourcesPath
     * @param targetJsonPath
     */
    static getFormJsonArrByTargetResource(targetModuleOptObj: ModuleOptObj, targetResourcesPath: string, targetJsonPath: string): string[];
    private static formJsonFileCheck;
    /**
     * 获取startup文件名
     */
    getAppStartupFileName(isFaMode: boolean): string | undefined;
    /**
     * 获取appStartup配置文件路径
     * @private
     */
    getAppStartupPath(isFaMode: boolean): string | undefined;
    /**
     * 获取指定根模块的shortcutsJson配置路径
     * @private
     * @param moduleModel
     */
    getShortcutsJsonPaths(moduleModel: ModuleModel | undefined): string[];
    /**
     * 获取shortcuts配置文件名
     * @param moduleModel
     * @private
     */
    getShortcutsJsonFileNames(moduleModel: ModuleModel | undefined): string[];
    /**
     * 处理能力元数据，提取快捷方式资源并添加到快捷方式列表中。
     *
     * @param abilities - 包含能力信息的数组，每个能力对象可能包含元数据。
     * @param shortcutsList - 用于存储提取的快捷方式资源的数组。
     */
    private handleAbilityMetadata;
    /**
     * 获取指定根模块的shareConfig配置路径
     * @private
     * @param moduleModel
     */
    getShareConfigJsonPath(moduleModel: ModuleModel | undefined): string | undefined;
    /**
     * 获取模块共享配置文件名
     * @param moduleModel
     * @private
     */
    getShareConfigJsonFileName(moduleModel: ModuleModel | undefined): string | undefined;
    /**
     * 根据target定制资源目录获取对应ShareConfigObjList
     * @param moduleModel
     * @private
     */
    getTargetShareConfigObjList(moduleModel: ModuleModel): ShareConfigObj[] | undefined;
    /**
     * 获取指定根模块的routerMap配置路径
     * @private
     * @param moduleModel
     */
    getRouterMapJsonPath(moduleModel: ModuleModel | undefined): string | undefined;
    /**
     * 获取当前根模块的InsightIntentJson配置路径
     * 存在自定义多资源目录的时候拿第一个（与routerMap保持一致）,否则返回默认路径base/profile/insightIntent.json
     * @private
     */
    getInsightIntentJsonPath(): string;
    /**
     * 获取模块路由表配置文件名
     * @param moduleModel
     * @private
     */
    getRouterMapJsonFileName(moduleModel: ModuleModel | undefined): string | undefined;
    /**
     * 根据target定制资源目录获取对应routerMapObjList
     * @param moduleModel
     * @private
     */
    getTargetRouterMapObjList(moduleModel: ModuleModel): RouterMapObj[] | undefined;
    /**
     * 获取target定制资源base/profile下的目录地址
     */
    getTargetResourceBaseProfilePath(relativePath: string): string | undefined;
    /**
     * 获取模块对应pkgContextInfo
     * @param packageName 包名
     */
    getPkgContextInfo(packageName: string): any;
    /**
     * 获取原始srcEntry的绝对路径
     * srcEntry在编辑器中限制只能以./开头,即相对于module.json
     * @param originalSrcEntry
     * @param rootPath
     */
    getAbsoluteSrcEntryPath(originalSrcEntry: string, rootPath: string): string;
    /**
     * 是否开启har包去重功能
     * 1.deduplicateHar开关打开
     * 2.preview,ohosTest,localTest场景不开启
     */
    shouldEnableHarDeduplicate(): boolean;
    /**
     * 单独调试hsp的场景是否需要执行去重任务
     * 除了要满足shouldEnableHarDeduplicate()条件,调试hsp时是否真正执行去重任务还要看withDeduplicate参数，withDeduplicate参数是调试传过来的
     */
    private shouldDoDeduplicateTaskInDebuggingHsp;
    /**
     * 是否需要真正执行去重相关任务(去重任务已注册，但有些场景如单独调试hsp时不需要执行任务)
     */
    shouldDoDeduplicateTask(): boolean;
    /**
     * 判断当前target是不是ohosTest
     * @returns {boolean}
     * @protected
     */
    isOhosTest(): boolean;
    /**
     * 判断当前模块是不是集成态Hsp
     * @returns {boolean}
     * @protected
     */
    isIntegratedHsp(): boolean;
}
/**
 * 拼接不同target的任务名
 *
 * @param {string} targetName
 * @param {string} taskName
 * @return {string}
 */
export declare function computeTargetTaskName(targetName: string, taskName: string): string;
