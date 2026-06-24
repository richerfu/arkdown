/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/module/core-module-model-impl.d.ts
 */
import { Module } from '@ohos/hvigor';
import { CompileModeEnum } from '../../enum/compile-mode-enum';
import { ModuleType } from '../../enum/module-type-enum';
import { ModuleBuildProfile } from '../../options/build/module-build-profile';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { ConfigJson } from '../../options/configure/config-json-options';
import { ModuleJson } from '../../options/configure/module-json-options';
import { ApiType } from '../../project/data/hap-extra-info';
import { ProjectModel } from '../project/project-model';
import { SourceSetModel, SourceSetModelType } from '../source-set/source-set-model';
import { ModuleModel } from './module-model';
import ConfigOptObj = ConfigJson.ConfigOptObj;
import { BuildOpt } from '../../options/build/build-opt';
import ProductApiMeta = ProjectBuildProfile.ProductApiMeta;
import ApiMeta = ProjectBuildProfile.ApiMeta;
import { ArkUIXConfigJson } from '../../options/configure/arkui-x-config-opt';
import { ModuleTaskService } from '../../tasks/service/module-task-service';
import { JsonProfile } from '../json-profile';
/**
 * hvigor 工程的module模块的数据管理对象
 *
 * @since 2022/2/23
 */
export declare abstract class CoreModuleModelImpl implements ModuleModel {
    private readonly module;
    private readonly name;
    private readonly parentProject;
    private readonly buildProfilePath;
    private moduleBuildOpt;
    private belongProjectPath;
    private compilePluginPath;
    protected readonly _sourceRootDir: string;
    protected readonly modulePath: string;
    protected targetSourceSetMap: Map<string, SourceSetModelType>;
    protected targets: ModuleBuildProfile.ModuleTargetBuildOpt[];
    protected constructor(module: Module, parentProject: ProjectModel);
    /**
     * 初始化belongProjectPath
     *
     * @param modulePath 模块路径
     */
    initBelongProjectPath(modulePath: string): string;
    findBelongProjectPath(currentDirPath: string): string;
    getBelongProjectPath(): string;
    getCompilePluginPath(): string | undefined;
    setCompilePluginPath(path: string): void;
    refreshData(): void;
    trace(): void;
    isOhpmProject(): boolean;
    abstract isAtomicService(): boolean;
    abstract isFormWidgetModule(targetName: string): boolean;
    abstract isFormExtensionModule(targetName: string): boolean;
    abstract isInstallFree(): boolean;
    abstract getDeviceTypes(): string[];
    abstract getPermission(): object;
    abstract hasFormConfig(targetName: string): boolean;
    isMixedDeviceModule(): boolean;
    isPurerRichDeviceModule(): boolean;
    haveRichDevicesInModule(): boolean;
    haveLiteDevicesInModule(): boolean;
    isPurerLiteDeviceModule(): boolean;
    isSingleDeviceTypeModule(): boolean;
    abstract getJsonObjByTargetName(targetName: string): ConfigOptObj | ModuleJson.ModuleOptObj;
    abstract initDefaultTargetSourceSet(): void;
    abstract getModuleType(): ModuleType;
    abstract getSourceSetByTargetName(targetName: string): SourceSetModel;
    abstract getJsonPathByTargetName(targetName: string): string;
    getRelatedEntryModules(): string[] | undefined;
    getSourceRootByTargetName(targetName?: string): string;
    getBuildProfileName(): string;
    getName(): string;
    getProjectDir(): string;
    getParentProject(): ProjectModel;
    getPackageJsonPath(): string;
    /**
     * 返回模块下的oh-package.json5路径 || .hvigor/dependencyMap下的oh-package.json5路径
     * @returns {string}
     */
    getOhPackageJson5Path(): string;
    getProfilePath(): string;
    getProfileOpt(): ModuleBuildProfile.ModuleBuildOpt;
    isArkModule(): boolean;
    /**
     * 根据模块的运行时配置文件module.json5/config.json来获取取模块的类型信息
     *
     * @returns {boolean} true/false
     */
    isHapModule(): boolean;
    isHarModule(): boolean;
    isHspModule(): boolean;
    getProductApiMeta(product: string): ProductApiMeta | undefined;
    getCompileApiMetaByProduct(product: string): ApiMeta;
    getCompatibleApiMetaByProduct(product: string): ApiMeta;
    /**
     * 返回compatibleSdkVersion原始值(用户配置的值)
     *
     * @param product
     * @returns {string}
     */
    getOriginCompatibleSdkVersionByProduct(product: string): string | number | undefined;
    getTargetApiMetaByProduct(product: string): ProjectBuildProfile.ApiMeta | undefined;
    getAllModules(): ModuleModel[];
    getApiType(): ApiType;
    /**
     * 模块的compileMode根据工程的属性进行判断
     *
     * @returns {CompileModeEnum} jsBundle | esModule
     */
    getCompileMode(product: ProjectBuildProfile.ProductBuildOpt): CompileModeEnum;
    getTargetOptions(): ModuleBuildProfile.ModuleTargetBuildOpt[];
    /**
     * 获取对应module的targets集合，如果没有配置，则默认为
     * [
     *  {
     *    name: 'default'
     *  }
     * ]
     *
     * @return {ModuleBuildProfile.ModuleTargetBuildOpt[]}
     */
    private getOrDefaultTargets;
    moduleBuildProfileCheck(isHapModule: boolean): void;
    /**
     * 模块的 targets -> resource -> directories中使用了绝对路径时进行告警。
     * @private
     */
    private warnIfUseAbsolutePath;
    private checkModuleBuildProfile;
    private validateCompileMode;
    getModule(): Module;
    processTargetsConfig(): void;
    /**
     * 返回当前模块的绝对路径化的worker路径，校验移至prebuild完成
     *
     * @param {BuildOpt | undefined} buildOption
     * @returns {string[] | undefined}
     */
    getWorkerConfig(buildOption: BuildOpt | undefined): string[] | undefined;
    getAllTargetSourceSet(): ReadonlyMap<string, SourceSetModelType>;
    getJsonProfilePath(targetName: string, isFaMode?: boolean): string;
    getJsonProfileOptByTargetName(targetName: string, isFaMode?: boolean): ConfigJson.ConfigOptObj | ModuleJson.ModuleOptObj;
    /**
     * 通过模型获取jsonProfile
     *
     * @param service moduleTaskService
     * @param targetName targetName
     * @private
     */
    getJsonProfileByModel(service: ModuleTaskService, targetName: string): JsonProfile;
    isCrossplatformModule(arkUIXConfigObj: ArkUIXConfigJson.ConfigObj | undefined): boolean;
    isIgnoreCrossPlatformCheckModule(arkUIXConfigObj: ArkUIXConfigJson.ConfigObj | undefined): boolean;
    /**
     * 获取远程仓hsp路径
     */
    getRemoteHspPath(): string;
    getMockConfigPath(): string;
    isOutModule(): boolean;
}
