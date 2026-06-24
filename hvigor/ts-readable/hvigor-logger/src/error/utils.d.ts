/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/utils.d.ts
 */
import { HvigorErrorAdaptor } from './adaptor/hvigor-error-adaptor';
import { ToolErrorCode } from './type';
/**
 * 获取错误信息文件的文件名
 * @param {string} code 两位数字的错误码
 */
export declare function getErrorInfoFilePath(code?: ToolErrorCode | string): string;
/**
 * 错误码格式化方法，导出
 * @param errorId
 * @param messages
 * @param solutions
 */
export declare function formatErrorAdaptorExport(errorId: string, messages?: unknown[], solutions?: unknown[][]): HvigorErrorAdaptor;
/**
 * 从error信息中的code得到description
 * @param errorCode
 */
export declare function errorCode2Description(errorCode: string): string;
