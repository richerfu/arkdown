/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/task-service.d.ts
 */
import { HvigorCoreNode } from '@ohos/hvigor';
import { DependencyInfoType, OhExportsDependencyInfo } from '@ohos/hvigor-arkts-compose';
import { ProjectModel } from '../../model/project/project-model';
import { DependRemoteHspInfo, OhpmDependencyInfo } from '../../plugin/context/plugin-context';
import { Dependency, DependencyType } from '../../project/dependency/core/dependency-interface';
import { DependencyManager, ModulePkgNode } from '../../project/dependency/dependency-manager';
import { ModuleDependencyInfo } from '../../project/dependency/module-dependency-info';
import { ModuleTaskService } from './module-task-service';
import { ProjectTaskService } from './project-task-service';
declare enum DependencyCacheKey {
    AllHspDependencies = 0,
    RootByteCodeHarDependencies = 1,
    RootOtherDependencies = 2,
    RootHspDependencies = 3,
    AffectsCompilationDependencies = 4,
    AllByteCodeHarDependencies = 5,
    AllModuleDependencies = 6
}
interface DependencyCacheValue {
    dependencies: Dependency[];
    moduleDepMap: Map<string, Dependency>;
}
/**
 * 任务流服务的基础类
 *
 * @since 2022/1/20
 */
export declare class TaskService {
    private readonly log;
    protected hvigorNode: HvigorCoreNode;
    protected readonly _projectModel: ProjectModel;
    private _moduleDependencyInfo;
    protected _dependencyManager: DependencyManager;
    protected _isFaMode: boolean;
    private _dependencyCache;
    constructor(node: HvigorCoreNode, projectModel: ProjectModel, dependencyManager: DependencyManager, isFaMode: boolean);
    getDependencyCache(): Map<DependencyCacheKey, Dependency[] | DependencyCacheValue>;
    initDependencyInfo(): void;
    isFaMode(): boolean;
    getNode(): HvigorCoreNode;
    getProjectModel(): ProjectModel;
    /**
     * 获取所有依赖的main对应文件的路径的集合
     * @returns {string[]}
     */
    getDependencyMainPaths(): string[];
    /**
     * 获取所有依赖在node_modules中路径的集合
     * @returns {string[]}
     */
    getDependencyRootPaths(): string[];
    /**
     * 获取不排除Hsp的间接依赖和DevDependencies的所有依赖
     */
    getAllModuleOrProjectDependencies(): Dependency[];
    /**
     * 获取不排除Hsp的间接依赖和DevDependencies的所有三方依赖
     */
    getAllOtherDependencies(): Dependency[];
    getDependencyName2RootPath(): any;
    getDependencyName2DepInfo(): Map<string, DependencyInfoType>;
    /**
     * 搜集每个依赖包里配置的oh-exports
     * 对于没有配置oh-exports的包map中不搜集不校验，和之前逻辑保持一致
     * @return 包名->导出组成的map
     */
    getDependencyName2OhExportsMap(): Map<string, OhExportsDependencyInfo>;
    /**
     * 搜集一个依赖中配置的 oh-exports 文件路径
     * @param dep 依赖
     * @private
     */
    private collectDependencyOhExports;
    /**
     * 获取所有har依赖
     * @returns {Dependency[]}
     */
    getHarDependencies(): Dependency[];
    /**
     * 获取远程har依赖
     */
    getRemoteHarDependencies(): Dependency[];
    /**
     * 获取所有hsp依赖
     * @returns {Dependency[]}
     */
    getHspDependencies(): Dependency[];
    /**
     * 获取所有依赖类型map
     */
    getAllDependencyType(): Map<string, DependencyType>;
    /**
     * 获取字节码Har对应的Hsp map: Map<dependencyKey, hspPkgName[]>
     */
    getByteCodeHar2Hsp(): Map<string, string[]>;
    /**
     * 递归获取依赖树中的所有hsp
     */
    getHspRecursion(deptName: string, allDep: Dependency[], tempMap?: Map<string, string[]>): string[];
    getHspOrByteCodeHarDependencies(): Dependency[];
    /**
     * 获取所有so依赖
     * @returns {Dependency[]}
     */
    getSODependencies(): Dependency[];
    /**
     * 获取所有ohpm npm依赖
     * @returns {Dependency[]}
     */
    getOtherDependencies(): Dependency[];
    /**
     * 获取所有其他依赖
     * @returns {Dependency[]}
     */
    getAllDependencies(): Dependency[];
    private excludeChildrenWhenInDevDependencies;
    getDirectPkgNodes(): ModulePkgNode[];
    /**
     * 获取当前模块的直接依赖
     * @returns {Dependency[]}
     */
    getDirectDependencies(): Dependency[];
    /**
     * 获取所有直接依赖子节点，包括devDependencies
     */
    getAllDirectPkgNodes(): ModulePkgNode[];
    /**
     * 获取所有直接依赖，包括devDependencies，工程级本地依赖
     */
    getAllDirectDependencies(): Dependency[];
    /**
     * 组建依赖树进行遍历收集，复用逻辑，调用_dependencyManager.collectFromTree
     * @param shouldTerminate 是否需要终止遍历，类型虽与_dependencyManager.collectFromTree一致，但返回值却被取反
     * @param shouldCollect 是否需要收集进最终的返回结果
     * @private
     */
    private collectFromTree;
    /**
     * 获取依赖链上的所有HSP，包括devDependencies，工程级本地HSP
     */
    getAllHspDependencies(): Dependency[];
    /**
     * 收集首层字节码HAR依赖，包括配置于devDependencies，工程级本地依赖的间接字节码HAR依赖
     * 只收集直接依赖中的字节码HAR，和被源码HAR依赖的字节码HAR
     * 1.HSP -> bytecodeHar，bytecodeHar不收集
     * 2.byteCodeHar -> dayjs，dayjs不收集
     * 3.byteCodeHar1 -> byteCodeHar2，byteCodeHar2不收集
     * 4.globalLocalHarDependency -> byteCodeHar，byteCodeHar收集
     */
    getRootByteCodeHarDependencies(): Dependency[];
    /**
     * 收集首层NPM三方包依赖，包括配置于devDependencies，工程级本地依赖的间接NPM三方包依赖
     * 只收集直接依赖中的NPM三方包，和被源码HAR依赖的NPM三方包
     * 1.HSP -> dayjs，dayjs不收集，被hsp依赖的，不收集
     * 2.byteCodeHar -> dayjs，dayjs不收集，被字节码HAR依赖的不收集
     * 3.dayjs -> otherNpmPackage，otherNpmPackage不收集
     * 4.globalLocalHarDependency -> dayjs，dayjs收集
     */
    getRootOtherDependencies(): Dependency[];
    /**
     * 收集首层HSP依赖，包括配置于devDependencies，工程级本地依赖
     * 1.HSP1 -> HSP2，HSP2不收集
     */
    getRootHspDependencies(): Dependency[];
    /**
     * 在目前的编译逻辑中，在编译期间rollup有可能需要对以下依赖进行编译阻断：
     * 1.NPM三方包（bundled字节码HAR）
     * 2.HSP（所有模块）
     * 3.字节码HAR（所有模块）
     * 所以，当前这个函数，收集依赖逻辑如下（依赖包括配置于devDependencies，工程级本地依赖）：
     * 1.bytecodeHar -> any，bytecodeHar收集，依赖any不收集
     * 2.sourcecodeHar1 -> sourcecodeHar2 -> dayjs -> any，sourcecodeHar1，sourcecodeHar2，dayjs收集，依赖any不收集
     * 3.HSP -> any，HSP收集，依赖any不收集
     * 4.sourcecodeHar1 -> sourcecodeHar2 -> bytecodeHar -> any，sourcecodeHar1，sourcecodeHar2，bytecodeHar收集，依赖any不收集
     * 简单点理解，就是，当某个依赖可能需要被编译阻断时，只收集自身，其依赖（直接/间接）都不收集。
     */
    getAffectsCompilationDependencies(): Dependency[];
    /**
     * 收集所有字节码HAR依赖，包括配置于devDependencies，工程级本地依赖
     */
    getAllByteCodeHarDependencies(): Dependency[];
    /**
     * 收集当前模块依赖链上的所有本地模块，包括devDependencies和工程级的本地依赖，被HSP阻断的不收集
     */
    getAllModuleDependencies(): Dependency[];
    /**
     * 获取所有hsp依赖的路径的集合
     * @returns {Dependency[]}
     */
    getHspDependencyPaths(): string[];
    /**
     * 获取所有本地鸿蒙依赖模块
     * @returns {Dependency[]}
     */
    getModuleDependencies(): Dependency[];
    /**
     * 获取所有本地鸿蒙依赖模块的路径的集合
     * @returns {Dependency[]}
     */
    getModuleDependenciesPaths(): string[];
    /**
     * 获取所有本地依赖har模块
     * @returns {[string, Dependency][]}
     */
    getHarModuleDependencies(): [string, Dependency][];
    /**
     * 获取所有本地依赖har模块返回Map
     * @returns {Map<string, [string, Dependency]>}
     */
    getHarModuleDependenciesWithRootPath(): Map<string, [string, Dependency]>;
    /**
     * 获取所有本地依赖har模块的名称
     * @returns {string[]}
     */
    getHarModuleDependencyNames(): string[];
    /**
     * 获取所有本地依赖har模块的路径
     * @returns {string[]}
     */
    getHarModuleDependencyPaths(): string[];
    /**
     * 获取所有本地依赖hsp模块
     * @returns {[string, Dependency][]}
     */
    getHspModuleDependencies(): [string, Dependency][];
    /**
     * 获取所有本地hsp依赖模块的名称
     * @returns {string[]}
     */
    getHspModuleDependencyNames(): string[];
    /**
     * 获取所有本地hsp依赖模块的路径
     * @returns {string[]}
     */
    getHspModuleDependencyPaths(): string[];
    /**
     * 获取除Dynamic所有本地hsp依赖模块的路径
     * @returns {string[]}
     */
    getHspModuleDependencyPathsWithOutDynamic(): string[];
    /**
     * 判断本模块是否有hsp依赖
     * @returns {[string, Dependency][]}
     */
    hasHspDependencies(): boolean;
    getDependencyInfo(): ModuleDependencyInfo;
    getOhpmDependencyInfo(): Record<string, OhpmDependencyInfo>;
    getOhpmRemoteHspAllInfo(service: ModuleTaskService | ProjectTaskService, currentProduct: string | undefined, isSigned: boolean): Record<string, DependRemoteHspInfo>;
    private setDependencyInfo;
    /**
     *   获取dependencies中的远程hsp依赖,对外暴露的api
     *
     * @param service
     * @param currentProduct
     * @param isSigned
     */
    getOhpmRemoteHspDependencies(service: ModuleTaskService | ProjectTaskService, currentProduct: string | undefined, isSigned: boolean): Record<string, OhpmDependencyInfo>;
    getRemoteHspDirName(dependencyRootPath: string): string | undefined;
}
export {};
