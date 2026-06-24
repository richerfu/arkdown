/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/hvigor-trace.d.ts
 */
import { AdaptorErrorMessage } from '@ohos/hvigor-logger';
declare class HvigorTrace {
    static readonly TRACE_KEY = "HVIGOR";
    private data;
    private configBlacklist;
    constructor();
    /**
     * 传输数据给打点管理中心并且在落盘后清理数据
     */
    transmitDataToManager(): void;
    traceTotalTime(totalTime: number): void;
    /**
     * 记录并行、增量、daemon、类型检查、分析模式的配置的值
     */
    traceBaseConfig(IS_INCREMENTAL: boolean, IS_DAEMON: boolean, IS_PARALLEL: boolean, IS_HVIGORFILE_TYPE_CHECK: boolean): void;
    traceBuildAnalysisMode(BUILD_ANALYSIS_MODE: string | undefined | boolean): void;
    /**
     * 记录hvigor配置enable ohpm execution by hvigor的配置值
     * @param IS_EXECUTION_OHPM_BY_HVIGOR
     */
    traceExecutionByOhpm(IS_EXECUTION_OHPM_BY_HVIGOR: boolean): void;
    traceBuildId(buildId: string): void;
    traceCustomPluginTime(pluginId: string | undefined, totalTime: number): void;
    traceTaskTime(taskName: string, moduleName: string, time: number): void;
    traceCustomTaskTime(taskName: string, moduleName: string, time: number): void;
    traceErrorMessage(adaptorErrorMessage: AdaptorErrorMessage): void;
    insertUsedApi(apiName: string): void;
    traceConfigProperties(configProperties: {
        [key: string]: any;
    }): void;
    traceExperimentFeature(name: string, value: boolean | string): void;
}
export declare const hvigorTrace: HvigorTrace;
export {};
