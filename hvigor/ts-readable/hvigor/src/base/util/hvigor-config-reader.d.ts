/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/hvigor-config-reader.d.ts
 */
import { Json5Reader } from './json5-reader';
export interface Environment {
    nodeHome?: string;
    hvigorUserHome?: string;
}
export interface Execution {
    daemon?: boolean;
    parallel?: boolean;
    incremental?: boolean;
    typeCheck?: boolean;
    analyze?: string | boolean;
    skipNativeIncremental?: boolean;
    optimizationStrategy?: string;
}
export interface Debugging {
    stacktrace?: boolean;
}
export interface Logging {
    level?: string;
}
export interface NodeOptions {
    maxOldSpaceSize?: number | undefined;
    maxSemiSpaceSize?: number | undefined;
    exposeGC?: boolean;
}
export interface NodeOptionSrc {
    argv: number | undefined;
    config: number | undefined;
    execArgv: number | undefined;
    default: number;
}
export interface JavaOptions {
    Xmx?: number;
}
export declare const defaultOptions: Required<NodeOptions>;
export interface HvigorConfig {
    modelVersion: string;
    dependencies: Record<string, string>;
    environment?: Environment;
    execution?: Execution;
    logging?: Logging;
    debugging?: Debugging;
    nodeOptions?: NodeOptions;
    javaOptions?: JavaOptions;
    properties?: Record<string, any>;
    parameterFile?: string;
}
/**
 * hvigor-config.json5读取器
 *
 * @since 2023/04/06
 */
export declare class HvigorConfigReader extends Json5Reader {
    private static readonly maxOldSpaceSizeParamPrefiex;
    private static readonly maxSemiSpaceSizeParamPrefiex;
    private static readonly STACK_TRACE;
    private static readonly NO_STACK_TRACE;
    static getHvigorConfig(): HvigorConfig | undefined;
    /**
     * 返回处理后的BuildParamFilePath
     */
    static getBuildParamFilePath(): string | undefined;
    /**
     * parameterFile路径 仅支持相对路径
     * @param parameterFilePath hvigor-config.json5被配置的路径
     * @param folderPath hvigor-config.json5所在目录路径
     */
    private static normalizeParameterFilePath;
    static getPropertiesConfigValue<T>(key: string): T | undefined;
    static getMaxOldSpaceSize(trigByConfigChange?: boolean): number;
    /**
     * 获取生效的stacktrace
     * @param {boolean} trigByConfigChange 配置变化触发标志，如果为true，则只返回src.config或src.default中的一个
     * @returns {boolean}
     */
    static getStacktrace(trigByConfigChange?: boolean): boolean;
    static getMaxSemiSpaceSize(trigByConfigChange?: boolean): number;
    static getNodeParamFromProcessArgv(): string[];
    /**
     * 优先级顺序：命令行参数 > hvigor-config中的参数 > hvigorw的.bat或者shell文件 > 默认值
     * 如果是检测到config配置改变导致的重启，则优先级： hvigor-config中的参数 > 默认值
     * 之所以进行判断，处理场景为第一次通过daemon启动后，第二次编译前注释掉config文件下的参数，默认值不会生效的问题。
     * 不做判断的话，当参数写入execArgv，重启不会使默认值生效
     * @param src 来源
     * @param trigByConfigChange 配置变化触发标志，如果为true，则只返回src.config或src.default中的一个
     * @return 返回src中的一个值
     */
    private static getPriorVal;
}
