/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/trace/ohos-trace.d.ts
 */
import { ModuleType } from '../../enum/module-type-enum';
import { BuildOpt } from '../../options/build/build-opt';
import { OhPackageJsonOpt, PackageJsonOpt } from '../../tasks/task/ohos-har-task';
import { RestoolCompressionTrace, TraceIncrement } from './type';
declare class OhosTrace {
    static readonly TRACE_KEY = "HVIGOR_OHOS_PLUGIN";
    private data;
    constructor();
    /**
     * 构建结束时打点
     */
    initOhosTrace(): void;
    private static getDefaultOhosTraceData;
    /**
     * 传输数据给打点管理中心并且在落盘后清理数据
     */
    transmitDataToManager(): void;
    traceBuildMode(buildMode: string): void;
    traceNativeCompiler(compiler?: string): void;
    /**
     * 以替换的方式记录模块数据
     * @param {string} moduleName
     * @param {string} apiType
     */
    traceModules(moduleName: string, apiType?: string): void;
    traceModuleType(moduleName: string, moduleType: ModuleType): void;
    /**
     * 获取打点数据里的模块数据
     * @param {string} moduleName
     * @returns {ModuleData | undefined}
     * @private
     */
    private getModule;
    traceRestoolCompression(moduleName: string, restoolCompressionReport: RestoolCompressionTrace): void;
    traceIncrement(moduleName: string, field: TraceIncrement, hasIncrement: boolean): void;
    traceIncludeInBuild(moduleName: string, includeInBuild: boolean): void;
    traceIsUseCompilePlugin(moduleName: string | undefined): void;
    traceIsUseTransformLib(moduleName: string | undefined): void;
    traceByteCodeHar(moduleName: string | undefined): void;
    traceUseNormalizedOHMUrl(useNormalizedOHMUrl?: boolean): void;
    traceIsAutoLazyImport(moduleName: string | undefined, isAutoLazyImport?: boolean): void;
    traceIsUsePacJson(moduleName: string | undefined): void;
    traceBuildOption(moduleName: string | undefined, buildOption: BuildOpt): void;
    tracePackageJson(moduleName: string | undefined, packageJson: PackageJsonOpt | OhPackageJsonOpt): void;
    traceProjectPackageJson(packageJson: PackageJsonOpt | OhPackageJsonOpt): void;
    /**
     * 一次构建中，一个模块是增量/全量的计算逻辑为INCREMENTAL_TASKS下所有任务都走了增量，这个模块才算是走了增量
     * @private
     */
    private traceModuleIncremental;
    /**
     * 全量构建标志isFullBuild初始为true，只要本次构建中只要有一个入口模块走了增量，则认为该构建为增量构建，将isFullBuild设为false
     * @private
     */
    private traceFullBuild;
    /**
     * 1. 如果命令行传了-p module，认为是入口模块
     * 2. 参与编译，根据模块类型对应assembleHap=>entry/feature, assembleHsp=>shared.assembleHar=>har,assembleApp=>entry/feature/shared为入口模块
     * @param module
     * @private
     */
    private isEntryModule;
}
export declare const ohosTrace: OhosTrace;
export {};
