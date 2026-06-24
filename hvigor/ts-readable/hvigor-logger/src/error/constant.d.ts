/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/constant.d.ts
 */
import { MatchFieldType, ToolErrorCode } from './type';
export declare const ERROR_INFO_DIRCTORY_PATH: string;
/**
 * 错误码：存储错误信息的文件名
 */
export declare const ErrorCodeToFile: Record<ToolErrorCode, string>;
export declare const DEFAULT_ERROR_CODE = "00000000";
export declare const ERROR_NOT_MATCH = "The error information does not match.";
export declare const ERROR_MESSAGE_HIDDEN = "Error message is hidden for security reasons";
export declare const MATCH_FIELD_TYPES: MatchFieldType[];
export declare const EIGHT_DIGIT_ERROR_REGEXP: {
    PATTERN: string;
    FLAGS: string;
};
