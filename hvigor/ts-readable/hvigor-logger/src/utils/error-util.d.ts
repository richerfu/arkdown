/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/error-util.d.ts
 */
import { AdaptorErrorMessage } from '../error/type';
import { HvigorErrorInfo } from '../logger/hvigor-error-info';
import { THvigorErrorInfo } from '../types/error-log-types';
export declare class ErrorUtil {
    /**
     * 将传入的object对象 实例化成HvigorErrorInfo
     * @param error
     * @returns
     */
    static getRealError(error: HvigorErrorInfo | THvigorErrorInfo): HvigorErrorInfo;
    /**
     * 将传入的object对象 实例化成三段式
     * @param error description为空字符串时，错误码描述信息按类别展示
     * @returns {string}
     */
    static combinePhase(error: THvigorErrorInfo): string;
    /**
     * 从错误码截取错误归属码
     * @param {string} errorCode
     * @returns {string}
     */
    static getErrorTypeCodeFromErrorCode(errorCode: string | undefined): string | undefined;
    /**
     * 兼容八位错误码之前的接口：
     * 对于已知错误码只包含一条错误信息的数组，使用此函数提取第一个
     * @param adaptorErrorMessages 适配器错误消息数组
     * @returns 返回第一个AdaptorErrorMessage，如果数组为空，则返回一个空的AdaptorErrorMessage
     */
    static getFirstErrorAdaptorMessage(adaptorErrorMessages: AdaptorErrorMessage[]): AdaptorErrorMessage;
    /**
     * 判断错误是不是适配器错误
     * @param {Error} error
     * @returns {boolean}
     */
    static isAdaptorError(error: Error): boolean;
}
