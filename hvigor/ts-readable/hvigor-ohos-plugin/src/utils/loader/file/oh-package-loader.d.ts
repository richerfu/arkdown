/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/oh-package-loader.d.ts
 */
declare class OhPackageLoader {
    private readonly _log;
    isUpdatedOhPackageInfo: boolean;
    private isLoaded;
    isChangedParameterFile: boolean;
    OhPackagePathMap: Map<string, string>;
    UpdatePathToOriginMap: Map<string, string>;
    getDependenciesOpt(nodeOhPackagePath: string): any;
    clean(): void;
    getDevDependenciesOpt(nodeOhPackagePath: string): any;
    getDynamicDependenciesOpt(nodeOhPackagePath: string): any;
    getOhPackageJsonObj(nodeOhPackagePath: string): Record<string, any> | undefined;
    setDependenciesOpt(nodeOhPackagePath: string, dependencies: any): void;
    setDevDependenciesOpt(nodeOhPackagePath: string, devDependencies: any): void;
    setDynamicDependenciesOpt(nodeOhPackagePath: string, dynamicDependencies: any): void;
    getVersion(nodeOhPackagePath: string): any;
    setVersion(nodeOhPackagePath: string, updateVersion: string): void;
    /**
     * 判断是否需要执行ohpm install
     * 1.是否更新oh-package.json5文件
     * 2.是否更新parameterFile文件
     * 3.是否使用includeNode及excludeNodeByName等api动态修改配置
     */
    shouldDoOhpmInstall(): boolean;
    loadUpdatedOhPackageToDisk(targetName: string, newParameterFile?: string): void;
    /**
     * 更新 oh-package.json5 路径映射
     * @param paths
     * @private
     */
    private updateOhPackagePathMapping;
    /**
     * 处理工程级oh-package.json5到dependencyMap/oh-package.json5之间的映射
     * @param node HvigorNode, 这里传入的是 HVIGOR_PROJECT 类型的节点
     * @param dependencyMapDirPath  .hvigor/dependencyMap 目录
     * @param newParameterFile 可配置参数文件
     * @private
     */
    private processProjectDependencyMap;
    /**
     * 处理各个模块oh-package.json5到dependencyMap/{模块名}/oh-package.json5之间的映射
     * @param node HvigorNode, 这里传入的是 HVIGOR_MODULE 类型的节点
     * @param dependencyMapDirPath  .hvigor/dependencyMap 目录
     * @param dependencyMapJsonPath .hvigor/dependencyMap/oh-package.json5 文件
     * @param moduleNames 模块名称集合
     * @param dependencyMap 新创建的
     * @private
     */
    private processModuleDependencyMap;
    /**
     * 判断新内容相对于文件中的内容是否有变化
     * @param filePath 文件路径
     * @param newContent 新内容对象
     * @returns 是否需要写入
     */
    private isFileContentEqual;
    /**
     * 清理冗余的模块目录
     * @param dependencyMapDirPath dependencyMap目录路径
     * @param moduleNames 当前需要保留的模块目录集合
     */
    private cleanUpRedundantModules;
    getNodeOhPackagePath(path: string): string;
    private saveOhPackagePathInfoToLoader;
    private validateDependency;
    getOriginPathFromUpdatePath(paths: string[]): string[];
    setIsChangeParameterFile(targetName: string, newParameterFile?: string): void;
    /**
     * 获取overrides
     * @param nodeOhPackagePath
     */
    getOverrides(nodeOhPackagePath: string): any;
    /**
     * 设置overrides
     * @param nodeOhPackagePath
     * @param overrides
     */
    setOverrides(nodeOhPackagePath: string, overrides: any): void;
}
export interface DependencyMap {
    targetName: string | undefined;
    rootDependency: string;
    dependencyMap: Record<string, string>;
    modules: HvigorModule[];
    basePath: string;
}
type HvigorModule = {
    name: string;
    srcPath: string;
};
export declare const ohPackageLoader: OhPackageLoader;
export {};
