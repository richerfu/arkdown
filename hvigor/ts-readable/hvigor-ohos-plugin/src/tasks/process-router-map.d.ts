/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-router-map.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
export declare class ProcessRouterMap extends OhosHapTask {
    private readonly _log;
    private readonly compileEntry;
    private readonly routerMapFileName;
    private readonly intermediatesRouterMapObjs;
    private readonly intermediatesTempRouterMapFilePath;
    private readonly intermediatesToLoaderRouterMap;
    private readonly useNormalizedOHMUrl;
    private moduleName2Target;
    private hspNeedDeduplicateHarInfo;
    private allHspDepTargets;
    constructor(targetService: TargetTaskService);
    private initHarDeduplicateData;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    declareInputs(): Map<string, TaskInputValue>;
    beforeAlwaysAction(): Promise<void>;
    private doSelfRouterMapNameCheck;
    protected doTaskAction(): void;
    /**
     * 处理har模块的routerMap
     * @param harRouterMapObjList
     * @private
     */
    private processHarRouterMap;
    /**
     * 处理entry/feature/hsp模块的routerMap
     * @param hapOrHspRouterMapObjList
     * @private
     */
    private processHapOrHspRouterMap;
    /**
     * 处理模块自有routerMap
     * @param allRouterMapObjList
     * @param moduleSelfRouterMapObjList
     * @private
     */
    private processModuleSelfRouterMap;
    /**
     * 处理模块依赖的routerMap
     * @param allRouterMapObjList
     * @private
     */
    private processModuleDependencyRouterMap;
    private getLocalDependencyRouterMap;
    /**
     * 处理模块本地依赖的routerMap
     * @param localDependency 模块直接依赖/间接依赖
     * @param allRouterMapObjList
     * @param moduleFlag2RouterMapObjList
     * @private
     */
    private processLocalDependencyRouterMap;
    /**
     * 处理模块远程依赖的routerMap(对比本地依赖,远程依赖在打包上传时已回填pageModule字段,所以一定存在pageModule)
     * 模块远程依赖无需校验路由表配置 上传远程仓时已确保路由表配置正确
     * @param remoteDependency
     * @param allRouterMapObjList
     * @private
     */
    private processRemoteDependencyRouterMap;
    /**
     * bundleHAR依赖的源码har在temp_router_map.json中新加入一个packageName字段标识属于哪个模块
     * @param dependencyRouterMapObj
     * @param dependency
     * @private
     */
    private routerMapAddPackageName;
    /**
     * 校验本模块routerMap对象配置
     * @param routerMapObjList
     * @param moduleModel
     * @param isDependency
     * @private
     */
    private checkModuleRouterMapObj;
    /**
     * 获取模块pageSourceFile
     * @param moduleModel
     * @param routerMapObj
     * @private
     */
    private getModulePageSourceFile;
    /**
     * 校验直接/间接依赖的routerMap配置
     * @param dependency
     * @param routerMapObjList
     * @private
     */
    private checkDependencyRouterMap;
    /**
     * 校验routerMap集合中对象名称唯一
     * @param routerMapObjList
     * @private
     */
    private checkRouterMapObjName;
    /**
     * 获取当前工程bundleName
     * @private
     */
    private getBundleName;
    /**
     * 生成entry/feature/hsp模块ohmurl
     * @param moduleName
     * @param pageSourceFile
     * @private
     */
    private generateModuleOhmurl;
    /**
     * 生成本地/远程har模块ohmurl
     * @param hostModuleName
     * @param moduleName
     * @param pageSourceFile
     * @param isLocalDependency
     * @param remoteDependencyName
     * @private
     */
    private generateHarOhmurl;
    /**
     * hsp不用将pageSourceFile写入compileEntry
     * @param moduleType
     * @param pageSourceFile
     * @private
     */
    private pushCompileEntry;
    initTaskDepends(): void;
    /**
     * 获取新规格化的ohmUrl
     *
     * @param localDependency
     * @param pageSourceFile
     * @private
     */
    private getOhmUrlWithNormalize;
    /**
     * 将处理过后的routerMap配置集合保存在中间产物临时目录中
     * @param routerMapObjList
     * @param intermediatesTempRouterMapObjList
     * @private
     */
    private generateTempRouterMap;
    /**
     * 处理har包去重逻辑
     * @private
     */
    private processHarDeduplicate;
    /**
     * 编译hap时合并hsp搜集到的去重har包中的RouterMap
     * @param allRouterMapObjList
     * @param intermediatesTempRouterMapObjList loader-router-map
     * @private
     */
    private addDuplicateRouterMap;
    /**
     * 合并RouterMap
     * @param duplicateRouterMapFilePath 去重RouterMap中间文件路径
     * @param targetRouterMapObjList 要合并到的对象
     * @param predicate 存在谓词
     * @param hspName hsp名字
     * @private
     */
    private mergeRouterMap;
    /**
     * 搜集hsp中被去重的har的routerMap
     * @private
     */
    private collectDuplicateHarRouterMap;
    /**
     * 编译hap时把hsp搜集到的去重RouterMap文件作为输入
     * @param inputFiles 输入文件
     */
    private processHarDeduplicateInputFiles;
    /**
     * 添加输入文件
     * @param filePath 输入文件路径
     * @param inputFiles 输入对象
     * @private
     */
    private addInputFileIfExists;
}
