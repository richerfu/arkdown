/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/build-native-with-ninja.d.ts
 */
import { DurationEvent } from '@ohos/hvigor';
import { AbstractBuildNative } from './abstract-build-native';
import { ModuleTargetData } from './data/hap-task-target-data';
import { TargetTaskService } from './service/target-task-service';
/**
 * ohos native代码编译so任务
 *
 * @since 2021/1/21
 */
export declare class BuildNativeWithNinja extends AbstractBuildNative {
    private _log;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    buildCommand(abiFilter: string, targetData: ModuleTargetData, callback: Function, callbackInput: unknown[], subDurationEvent: DurationEvent): Promise<void>;
    taskShouldDo(): boolean;
    protected doTaskAction(): Promise<void>;
    cleanUnwanted(abiFilters: string[]): Promise<void>;
    /**
     * 根据过滤条件删除目录
     * @param basePath 基础路径
     * @param shouldRemove 返回true表示需要删除该目录，返回false表示保留
     */
    private removeDirectoriesByFilter;
    /**
     * 检查.so实际输出与默认输出并同步
     *
     * @param nativeLibraries NativeLibraryModel
     * @param expectedOutput 默认输出
     * @param cxxAbi .cxx中间目录
     * @private
     */
    private syncOutput;
    /**
     * 同步本模块生成的so产物到intermediates/cmake目录下
     *
     * @param library NativeLibraryModel
     * @param expectedOutput 默认输出
     * @param cxxAbi .cxx中间目录
     * @param index 全局同步的so缓存, 避免重复拷贝
     * @private
     */
    private syncLibOutputs;
    /**
     * 同步本模块生成的so的依赖so到intermediates/cmake目录下
     *
     * @param library NativeLibraryModel
     * @param expectedOutput 默认输出
     * @param cxxAbi .cxx中间目录
     * @param index 全局同步的so缓存, 避免重复拷贝
     * @private
     */
    private syncLibDeps;
    /**
     * 是否需要exclude
     * @param file
     * @private
     */
    private isExcludeFromHar;
    /**
     * 如果该so文件已存在于缓存中，并且需要exclude，需将缓存中的so文件删除
     * @param file
     * @param destination
     * @private
     */
    private removeHarOutPutSo;
    /**
     * 同步标准库stl
     *
     * 设置了 -DOHOS_STL=c++_static 或者 -DOHOS_STL=none 时，不拷贝 libc++_shared.so，且删除临时目录下的.so文件
     * 否则拷贝libc++_shared.so到临时目录
     * 最终打包到hap的lib目录下
     *
     * @param abiFilter abiFilter
     * @param outputDir 临时目录
     * @param replyFolder cmake file-api reply folder
     * @param pathInfo pathInfo
     * @private copyLibSo
     */
    private syncStl;
    private excludeFromHsp;
    private excludeFromHar;
    /**
     * 获取当前构建参数
     *
     * @private
     */
    private getCurrentBuildInfo;
    /**
     * 检查构建状态 buildMode 与 cmakeBuildType 与上一次缓存均不一致则清理 cmake\${target}\obj\${abi} 缓存
     *
     * @param pathInfo 模块路径信息
     * @param abiFilters
     * @private
     */
    private checkAndHandleBuildState;
    /**
     * 缓存构建参数 buildMode,cmakeBuildType 到 build_info.json 目录
     *
     * @param pathInfo 模块路径信息
     * @private
     */
    private cacheBuildInfo;
}
