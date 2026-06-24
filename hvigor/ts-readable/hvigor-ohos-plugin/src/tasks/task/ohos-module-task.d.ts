/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/ohos-module-task.d.ts
 */
import { CoreTask, HvigorCoreNode, IncrementalExecTask, Module, TaskDetails } from '@ohos/hvigor';
import { ModulePathInfoIml } from '../../common/iml/module-path-info-iml';
import { CoreModuleModelImpl } from '../../model/module/core-module-model-impl';
import { ProjectModel } from '../../model/project/project-model';
import { SdkInfo } from '../../sdk/sdk-info';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { ModuleTargetData } from '../data/hap-task-target-data';
import { ModuleTaskService } from '../service/module-task-service';
import { TargetTaskService } from '../service/target-task-service';
import { OhPackageJsonOpt, PackageJsonOpt } from './ohos-har-task';
import { Dependency } from '../../project/dependency/core/dependency-interface';
import { StartupOptions } from '../../options/configure/startup-options';
/**
 * openHarmony的公共module类型的基础task
 *
 * @since 2022/1/5
 */
export declare abstract class OhosModuleTask extends IncrementalExecTask {
    private readonly ohosLogger;
    readonly pathInfo: ModulePathInfoIml;
    moduleModel: CoreModuleModelImpl;
    targetName: string;
    targetData: ModuleTargetData;
    protected readonly projectModel: ProjectModel;
    protected module: Module;
    protected moduleName: string;
    protected service: ModuleTaskService;
    targetService: TargetTaskService;
    protected compileApiVersion: number;
    protected compatibleApiVersion: number;
    protected originCompatibleSdkVersion: string | number | undefined;
    protected compatibleSdkVersionStage: string | undefined;
    protected targetSdkVersion: number | undefined;
    protected sdkInfo: SdkInfo;
    protected isFaMode: boolean;
    protected readonly isOhpmProject: boolean;
    protected readonly packageManagerPath: string;
    protected readonly packageJsonObj: PackageJsonOpt | OhPackageJsonOpt;
    protected readonly paramWhiteList: string[];
    protected readonly moduleModelNodePaths: Set<string>;
    protected readonly appStartupPath: string | undefined;
    protected readonly appStartupFileName: string | undefined;
    protected readonly intermediateTempStartupFilePath: string;
    private preloadSystemSoJson;
    /**
     * 当 oh-package.json5 中的依赖是以 @param: 参数化形式填写的本地产物依赖
     * 在打包该 har 模块时需要把本地产物依赖也打进去，所以需要记录依赖的绝对路径，在 AbstractProcessHarArtifacts 中使用
     * 打包非 har 模块或没有使用 @param: 不会有变化
     * @protected
     */
    protected readonly localProductDependenciesPath: string[];
    protected constructor(targetService: TargetTaskService, taskDetails: TaskDetails);
    /**
     * preloadSystemSo是否开启
     * @protected
     */
    protected get isPreloadSystemSoEnabled(): boolean;
    /**
     * 如果系统So的收集文件（preload.json）不存在，或者里面的内容为空，此处返回false，后续的的逻辑就不需要处理：
     * 1.字节码HAR不需要处理preload.json的打包逻辑
     * 2.HAP的GeneratePkgModuleJson不要处理添加appStartup逻辑
     * 3.HAP的ProcessCompiledResources的doTaskAction可以直接跳过
     * 4.HAP的FastPackageApp可以跳过preload.json的合并逻辑
     * @protected
     */
    protected get isPreloadSystemSoJsonEffective(): boolean;
    protected getPreloadSystemSoJson(): undefined | StartupOptions;
    /**
     * 用于将系统So的收集文件（preload.json）内容，合并到启动框架配置文件中
     * @param preloadSoJson
     * @param targetDir
     * @protected
     */
    protected addPreloadSystemSoJson(preloadSoJson: StartupOptions, targetDir: string): void;
    private shouldCloseWorkerPoolAfterBuild;
    /**
     * 该方法用于添加本模块的任务与其他非library模块之间的任务依赖关系,和处理逻辑
     * 由于需要获取其他模块的信息,因此需要在hvigor的nodeEvaluated方法中先注册,等待所有模块evaluate完后,再执行
     *
     * @protected
     */
    protected initTaskDependsForOtherModule(): void;
    /**
     * 该方法只能添加本模块自身的task之间的任务依赖关系,其中的处理逻辑不需要依赖其他模块的任务数据
     *
     * @protected
     */
    abstract initTaskDepends(): void;
    /**
     * 该方法用于添加本模块与本地library模块之间的任务依赖关系
     *
     * @protected
     */
    protected initHarModuleDepends(): void;
    /**
     * 为每个target执行task的具体逻辑
     */
    protected abstract doTaskAction(): void;
    /**
     * 在task执行之前做hook, 通常用于clean上次构建的输出
     */
    protected beforeTask(): void;
    /**
     * 根据targetData状态判断是否跳过该task
     */
    taskShouldDo(): boolean;
    registryAction: () => Function;
    /**
     * 获取每个target的临时目录(缓存目录)
     * 调该方法时如果该目录不存在，则创建新目录
     *
     * @param {string} targetData targets数据对象
     * @param {string} tempDirName 拼接目录名称
     * @param {boolean} isCreateDir 若目录不存在，是否创建新目录
     * @protected
     */
    protected getTaskTempDir(targetData: ModuleTargetData, tempDirName?: string, isCreateDir?: boolean): string;
    protected getImporteesCachePath(cacheFileName: string, arkCompileName: string): string;
    /**
     * 根据编译期间收集到的importee，找到对应的依赖，并由回调进行处理
     * @param filePath
     * @param dependencies
     * @param callback
     * @protected
     */
    protected processImportees(filePath: string, dependencies: Dependency[], callback: Function): void;
    /**
     * 根据优先级合并后，获取全局依赖的对应的依赖别名
     * @protected
     */
    protected getGlobalDependencyKeys(): string[];
    /**
     * 收集参与编译的一些依赖信息
     * @protected
     * @return
     *   compileDependencyKeys: 参与编译的依赖别名（自身+工程级）
     *   globalCompileDependencyKeys：参与编译的依赖别名（工程级）
     */
    protected collectDirectAffectsCompilationDependencyKeys(importeesPath: string): {
        compileDependencyKeys: string[];
        globalCompileDependencyKeys: string[];
    };
    /**
     * 针对以下情况，需要做warning提示：
     * 1.编译HSP/bundled的字节码HAR，若使用到工程级本地HAR依赖，需要给出warning
     * @param importeesPath
     * @protected
     */
    protected doGlobalLocalDependencyCheck(importeesPath: string): void;
    /**
     * 针对以下情况，需要做warning提示：
     *  1.编译HSP/HSP/bundled的字节码HAR，若使用到devDependencies中的HAR依赖，需要给出warning
     * @param importeesPath
     * @protected
     */
    protected doDevDependencyCheck(importeesPath: string): void;
    /**
     * 替换.preview产物中的pages文件的src
     * 保证路由/非路由页面预览pages.json中配置一致
     *
     * @param {OhosLogger} log
     * @param {string[]} pages 预览传入当前实际预览页面src
     * @protected
     */
    protected replacePagesInPreview(log: OhosLogger, pages: string[]): void;
    /**
     * 替换module.json5中配置的pages信息
     * 该方法需要在stage模型的资源编译后才可调用
     *
     * @param {OhosLogger} log
     * @param {string[]} pages
     * @protected
     */
    protected replacePages(log: OhosLogger, pages: string[]): void;
    /**
     * 获取profile pages路由配置文件名
     *
     * @param {OhosLogger} log
     * @return {string} profile路由配置文件名称
     * @protected
     */
    protected getPageJsonFileName(log: OhosLogger): string;
    /**
     * 解析模块级/工程级的oh-package.json5/package.json获取其dependencies或devDependencies
     * @param model CoreModuleModelImpl | ProjectModel
     * @param needDev
     * @private
     */
    protected getDependencies(model: CoreModuleModelImpl | ProjectModel, needDev: boolean): Record<string, string>;
    protected getDynamicDependencies(model: CoreModuleModelImpl | ProjectModel): Record<string, string>;
    declareDepends(task: string | CoreTask, node?: string | HvigorCoreNode): CoreTask;
    declareDependsList(...tasks: (string | CoreTask)[]): void;
    dependsOnHook(task: string | CoreTask, node?: string | HvigorCoreNode): CoreTask;
    protected hasUseTsHarField(): boolean;
    /**
     * 获取模块 oh-package.json5中的 oh-exports 导出的绝对路径
     * @protected
     */
    protected getOhExportsFilePaths(): string[] | undefined;
}
