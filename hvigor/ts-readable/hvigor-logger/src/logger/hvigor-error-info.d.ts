/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-error-info.d.ts
 */
import { MoreInfo, THvigorErrorInfo } from '../types/error-log-types';
/**
 * hvigor的错误信息描述类。
 * 提供数据的输入、校验、获取。
 */
export declare class HvigorErrorInfo {
    private code;
    private description;
    private cause;
    private position;
    private solutions;
    private moreInfo?;
    constructor(errorInfo?: THvigorErrorInfo);
    getCode(): string;
    getDescription(): string;
    getCause(): string;
    getPosition(): string;
    getSolutions(): string[];
    getMoreInfo(): MoreInfo | undefined;
    /**
     * 校验HvigorErrorInfo数据是否合法
     * @returns
     */
    checkInfo(): boolean;
    /**
     * 校验错误码的8位数字组成的字符串
     * @returns
     */
    private checkCode;
    /**
     * 校验错误原因是否存在
     * @returns
     */
    private checkCause;
}
