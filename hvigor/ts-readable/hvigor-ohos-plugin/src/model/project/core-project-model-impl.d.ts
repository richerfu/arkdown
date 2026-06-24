/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/core-project-model-impl.d.ts
 */
import { Project } from '@ohos/hvigor';
import { TargetRuntimeOS } from '../../options/build/module-build-profile';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { ModuleModel } from '../module/module-model';
import { ProjectModel } from './project-model';
import { ArkUIXConfigJson } from '../../options/configure/arkui-x-config-opt';
import { TargetTaskService } from '../../tasks/service/target-task-service';
import { OhPackageJsonOpt, PackageJsonOpt } from '../../tasks/task/ohos-har-task';
import { CrossplatformInterface } from './arkui-x/crossplatform-interface';
/**
 * Stage或Fa模型的工程持久化数据模型,包含工程源码数据,配置数据等
 *
 * @since 2022/1/20
 */
export declare abstract class CoreProjectModelImpl implements ProjectModel, CrossplatformInterface {
    protected readonly project: Project;
    private readonly _log;
    private readonly productApiMetaMap;
    private readonly arkuiProjectModel;
    protected readonly projectPath: string;
    protected readonly name: string;
    protected readonly _profilePath: string;
    protected parameterFileAbsolutePath: string | undefined;
    protected parameterFileObj: object | undefined;
    protected readonly projectPackageJsonFilePath: string;
    protected readonly packageJsonObj: PackageJsonOpt | OhPackageJsonOpt;
    protected _profileOptions: ProjectBuildProfile.ProjectProfileOpt;
    protected _productNames: string[];
    protected _moduleSpecificTargets: Map<string, string[]>;
    protected subModels: Map<string, ModuleModel>;
    protected targetRuntimeOS: TargetRuntimeOS[];
    protected _moduleTargets: Map<string, TargetTaskService[]>;
    protected constructor(project: Project);
    private initOhpmParameterFile;
    trace(): void;
    refreshData(): void;
    registryTarget(service: TargetTaskService): void;
    getTarget(moduleName: string, targetName?: string): TargetTaskService | undefined;
    getTargetRuntimeOSs(): TargetRuntimeOS[];
    private parseConfiguredTargetsInHvigorCommand;
    private projectStatusCheck;
    /**
     * 初始化product的api版本信息
     *
     * @private
     */
    private initializeApiMetadata;
    /**
     * 获取该工程默认的bundleName值,即AppScope中配置的值
     */
    abstract getDefaultBundleName(): string;
    /**
     * 获取该工程默认的BundleType值
     */
    abstract getBundleType(): string;
    /**
     * FA/Stage模型初始化子模块
     */
    abstract initSubModules(): void;
    getBuildProfileName(): string;
    getName(): string;
    getProject(): Project;
    getPackageJsonPath(): string;
    getOhPackageJson5Path(): string;
    getProfilePath(): string;
    getProjectDir(): string;
    getProductNames(): string[];
    getProfileOpt(): ProjectBuildProfile.ProjectProfileOpt;
    getSubModuleModels(): Map<string, ModuleModel>;
    getModuleModelByName(moduleName: string): ModuleModel | undefined;
    /**
     * 根据product获取compileSdkVersion
     * 不要早于ProjectInspection使用, 可能获取无效值
     *
     * @param product
     */
    getCompileApiMetaByProduct(product: string): ProjectBuildProfile.ApiMeta;
    /**
     * 根据product获取compatibleSdkVersion
     * 不要早于ProjectInspection使用, 可能获取无效值
     *
     * @param product
     */
    getCompatibleApiMetaByProduct(product: string): ProjectBuildProfile.ApiMeta;
    /**
     * 返回compatibleSdkVersion原始值(用户配置的值)
     *
     * @param product
     * @returns {string}
     */
    getOriginCompatibleSdkVersionByProduct(product: string): string | number | undefined;
    getProductApiMeta(product: string): ProjectBuildProfile.ProductApiMeta | undefined;
    getTargetApiMetaByProduct(product: string): ProjectBuildProfile.ApiMeta | undefined;
    getAllModules(): ModuleModel[];
    getAllEntryModules(): Set<string>;
    /**
     * 获取根项目下build-profile.json5中的applyToProduct配置
     *
     * @param moduleName
     * @param targetName
     */
    getTargetApplyProducts(moduleName: string, targetName: string): string[] | undefined;
    /**
     * 根据moduleName获取build-profile.json5中的module配置
     *
     * @param moduleName
     */
    getModuleProfileOpt(moduleName: string): ProjectBuildProfile.ModuleBuildOpt | undefined;
    /**
     * 获取命令行参数-p module=配置的module和targets信息
     */
    getModuleSpecificTargets(): Map<string, string[]>;
    getParameterFileAbsolutePath(): string | undefined;
    getParameterFileObj(): object | undefined;
    private projectBuildProfileCheck;
    /**
     * 判断该工程是否为ohpm工程:
     * api高于9且工程目录下存在oh-package.json5(和工程管理逻辑保持一致)
     * api高于9且不存在存在oh-package.json5也不存在package.json，默认是ohpm工程
     *
     * @returns true or false
     */
    isOhpmProject(): boolean;
    /**
     * 获取远程仓hsp路径
     */
    getRemoteHspPath(): string;
    /**
     * 获取工程级的签名remote-hsp存储目录
     * @param productName
     */
    getCacheRemoteHspPath(productName: string): string;
    /**
     * 获取工程级的integrated_hsp存储目录
     * @param productName
     */
    getCacheIntegratedHspPath(productName: string): string;
    isFaMode(): boolean;
    isCrossplatformProject(): boolean;
    getArkUIXConfigJsonPath(): string;
    getArkUIXConfigJsonObj(): ArkUIXConfigJson.ConfigObj | undefined;
}
