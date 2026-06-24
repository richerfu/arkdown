/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/trace/type.d.ts
 */
import { JSON_VALUE_RECORD } from '@ohos/hvigor-common';
/**
 * hvigor-ohos-plugin项目的打点数据声明
 */
export interface OhosTraceData {
    BUILD_MODE?: string;
    MODULES: ModuleData[];
    USE_NORMALIZED_OHMURL?: boolean;
    NATIVE_COMPILER: string;
    IS_FULL_BUILD: boolean;
    PACKAGE_JSON: JSON_VALUE_RECORD[];
}
export interface ModuleData {
    MODULE_NAME: string;
    API_TYPE?: string;
    RESTOOL_COMPRESSION?: RestoolCompressionTrace;
    INCREMENTAL_TASKS?: Record<TraceIncrement, boolean>;
    IS_INCREMENTAL_MODULE?: boolean;
    INCLUDE_IN_BUILD?: boolean;
    IS_COMMAND_LINE_ENTRY_MODULE?: boolean;
    MODULE_TYPE?: string;
    IS_USE_COMPILE_PLUGIN?: boolean;
    IS_USE_TRANSFORMLIB?: boolean;
    IS_AUTO_LAZY_IMPORT?: boolean;
    IS_USE_PAC?: boolean;
    IS_BYTE_CODE_HAR?: boolean;
    BUILD_OPTION: JSON_VALUE_RECORD[];
    PACKAGE_JSON: JSON_VALUE_RECORD[];
}
/**
 * 纹理压缩打点
 */
export interface RestoolCompressionTrace {
    /** 时间 */
    TIMESTAMP: number;
    /** 转换总个数 */
    TRANSCODE_FILES: number;
    /** 转换成功率 */
    TRANSCODE_SUCCESS_RATE: number;
    /** 转换总耗时 */
    TRANSCODE_TOTAL_TIME: number;
    /** 转换总膨胀率 */
    TRANSCODE_EXPANSION_RATE: number;
}
export declare enum TraceIncrement {
    BUILD_NATIVE_WITH_CMAKE = "BUILD_NATIVE_WITH_CMAKE",
    COMPILE_ARKTS = "COMPILE_ARKTS"
}
