/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/file-util.d.ts
 */
export type Filter = string | string[] | RegExp | Function | null;
/**
 * 同步遍历文件夹下所有文件
 *
 * @param dirPath 要遍历的目录路径
 * @param filter 过滤条件
 * @returns 符合条件的文件路径数组
 */
export declare function traverseFilesSync(dirPath: string, filter?: Filter): string[];
/**
 * 判断文件是否应该被包含在结果中
 */
export declare function shouldIncludeFile(filePath: string, filter: Filter): boolean;
