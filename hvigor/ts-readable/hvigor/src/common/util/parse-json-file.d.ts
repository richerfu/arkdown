/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/parse-json-file.d.ts
 */
export declare class ParseJsonFile {
    static parseJsonFile(filePath: string, location?: boolean, encoding?: string): any;
}
/**
 * 防止其他库如arkuix引用导致版本不同引发的报错
 * @param filePath
 * @param location
 * @param encoding
 */
export declare function parseJsonFile(filePath: string, location?: boolean, encoding?: string): any;
