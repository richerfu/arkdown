/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/parse-json/judge-util.d.ts
 */
export declare class JudgeUtil {
    static isIgnoreChar(c: any): boolean;
    static isSpaceSeparator(c: any): boolean;
    static isIdStartChar(c: any): boolean;
    static isIdContinueChar(c: any): boolean;
    static isDigitWithoutZero(c: string): boolean;
    static isDigit(c: any): boolean;
    static isHexDigit(c: any): boolean;
}
