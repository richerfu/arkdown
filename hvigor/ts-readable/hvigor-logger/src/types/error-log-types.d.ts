/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/types/error-log-types.d.ts
 */
import { CountryEnum, LogLevel } from '../consts/errorlog-consts';
export type MoreInfo = Record<CountryEnum, string>;
export type THvigorWarnInfo = string;
export type THvigorInfoInfo = string;
export type THvigorDebugInfo = string;
export type THvigorErrorInfo = {
    /**
     * 匹配关键字，用于兜底场景
     */
    keys?: string[];
    /**
     * 错误码, 8位
     */
    code?: string;
    /**
     * 对错误码的描述
     */
    description?: string;
    /**
     * 错误原因，描述错误发生的前提，需针对发生错误的上下文具体描述
     */
    cause?: string;
    /**
     * 错误发生的位置，从用户视角指明哪个文件、哪个配置项，哪个API，哪个参数等报错
     */
    position?: string;
    /**
     * 具体的解决方法，一般为多条，一条方法为一个字符串
     */
    solutions?: string[];
    /**
     * 指导解决该错误的官网FAQ地址，格式：{cn: https://xxx, en: https://xxx}
     */
    moreInfo?: MoreInfo;
};
export type THvigorLogInfo = {
    /**
     * 通过log4j打印的信息
     */
    message: string;
    /**
     * 打印级别
     */
    logLevel: LogLevel;
};
