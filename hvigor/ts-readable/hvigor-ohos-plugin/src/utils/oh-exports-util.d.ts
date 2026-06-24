/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/oh-exports-util.d.ts
 */
export declare function rewriteOhExports(modulePath: string, packPath: string, ohExports: string[]): string[];
/**
 * 搜集 oh-exports 文件的绝对路径，对于文件夹要递归搜集下面所有的文件
 *
 * @param basePath 相对的base路径
 * @param ohExports oh-exports
 */
export declare function collectOhExportsFilePaths(basePath: string, ohExports: string[]): string[];
