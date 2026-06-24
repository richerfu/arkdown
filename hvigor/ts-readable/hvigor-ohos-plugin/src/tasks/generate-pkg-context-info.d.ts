/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-pkg-context-info.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { PkgContextInfoOpt } from '@ohos/hvigor-arkts-compose';
import { OhosLogger } from '../utils/log/ohos-logger';
import { ProcessLibsCommonProperties } from './common-properties/process-libs-common-properties';
import { TargetTaskService } from './service/target-task-service';
export declare const soLibsPattern: RegExp;
export declare class GeneratePkgContextInfo extends ProcessLibsCommonProperties {
    private readonly pkgContextInfoMap;
    protected _log: OhosLogger;
    private readonly currentBundleName;
    private readonly currentBundleType;
    private readonly pkgInfoPath;
    private duplicatePkgContextInfoObj;
    private allHspDepTargets;
    private get useNormalizedOHMUrl();
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    declareInputs(): Map<string, import("@ohos/hvigor").TaskInputValue>;
    constructor(taskService: TargetTaskService);
    beforeAlwaysAction(): Promise<void>;
    beforeTask(): void;
    taskShouldDo(): boolean;
    protected doTaskAction(): void;
    private initPkgContextInfo;
    /**
     * 获取当前编译模块的信息表
     * @private
     */
    private getCurrentPkgContextInfo;
    /**
     * 不同场景下获取当前模块的entryPath
     *
     * @param _dependencyRootPath
     * @param _pkgJsonObj
     * @private
     */
    private getCurrentEntryPath;
    /**
     * 获取依赖收集的数据组合出的信息表
     *
     * @param dependencyInfo
     * @private
     */
    private getDependenciesPkgContextInfo;
    /**
     * oh-exports转成为ohmurl格式写入到语境信息表中
     *
     * @param packageRootPath
     * @param ohExportsFilePaths
     * @param pkgContextInfo
     */
    private writeOhExportsOhmUrlToPkgContextInfo;
    /**
     * 为oh-exports的每条记录生成对应的ohmUrl
     * @param packageRootPath 包根路径
     * @param ohExportsFilePaths oh-exports 绝对路径
     * @param pkgContextInfo 语境信息表
     * @private
     */
    private generateOhExportsOhmUrlObj;
    /**
     * 检查给定的bundleType是否为SHARED或APP_PLUGIN，hsp则均需要回填bundleName
     * @return 如果bundletype是SHARED或APP_PLUGIN，则返回true，否则返回false
     * @param bundleType
     */
    private isBundleTypeSharedOrAppPlugin;
    /**
     * 应用间HspA 依赖应用间HspB 用HspB的bundle name
     * 应用间HspA 依赖HarB 用HspA的bundle name
     * 应用内Hsp 依赖应用内的包 不写入bundle name
     * 应用内Hsp 依赖应用间的hsp 如果bundle name不同则写入 否则不写入
     *
     * @private
     * @param dependencyInfo  依赖的包信息
     * @param dependencyType  依赖的包类型
     */
    private getBundleName;
    /**
     * 定制化获取version 只有har和ohpm包需要带version
     *
     * @private
     * @param dependencyInfo  依赖的包信息
     * @param dependencyType  依赖的包类型
     */
    private getVersion;
    /**
     * 获取别名，如果别名和pkgName不同则写入信息表
     *
     * @param dependencyInfo
     * @private
     */
    private getDependencyAlias;
    /**
     * 判断依赖的hsp是否为应用间hsp,此出因为插件包的需求，插件包在构建的视角也为应用间hsp，所以此处bundleType为shared和appPlugin均为应用间hsp
     *
     * @private
     * @param moduleJsonObj
     * @param dependencyType
     */
    private isHspInCommonApp;
    /**
     * 由于无法通过挂载processLibs任务去收集so，所以手动进行so的收集
     *
     * @private
     */
    private initLibsSoPkgContextInfo;
    /**
     * 循环遍历libs目录收集所有so文件
     *
     * @param libsPath
     * @private
     */
    private getNativeLibraryByLibs;
    /**
     * 通过targetService获取当前模块需要编译的so文件
     *
     * @param targetService
     * @private
     */
    private getNativeLibrary;
    /**
     * 将so相关文件写入信息表（如果配置了collectAllLibs则还会额外收集libs目录下的其他文件）
     *
     * @param soFileName
     * @private
     */
    private setPkgContextInfoMapWithSo;
    /**
     * 获取当前模块的bundleName， 应用间hsp的场景需要获取获取编译hsp时的bundleName
     *
     * @private
     */
    private getCurrentModuleBundleName;
    /**
     *  编译hap时,要把那些只被hsp依赖且需要去重的har信息合并到 hap的语境信息表中，因为 entry 中没有这些只被hsp依赖的har信息
     * 要搜集直接依赖和间接依赖的所有本地hsp
     */
    private addDuplicateHarPkgContextInfo;
    private processHspHarDeduplicate;
    /**
     * 记录要去重的语境信息到中间文件
     */
    private writeDuplicatePkgContextInfoToIntermediateFile;
    /**
     * 编译hap时把hsp搜集到的去重语境信息文件作为输入
     * 要搜集直接依赖和间接依赖的所有本地hsp
     */
    private processHarDeduplicateInputFiles;
    initTaskDepends(): void;
}
export declare function getSoDefaultContextInfo(soFileName: string, currentBundleName?: string): PkgContextInfoOpt;
