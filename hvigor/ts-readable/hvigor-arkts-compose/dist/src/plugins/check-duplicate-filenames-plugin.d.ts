/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/check-duplicate-filenames-plugin.d.ts
 */
export declare function checkDuplicatedFileNamesPlugin(): {
    name: string;
    load: typeof load;
    beforeBuildEnd: typeof beforeBuildEnd;
    cleanUp(): void;
};
/**
 * 对于load加载的文件，做一个文件绝对路径和去除后缀名的map，用来判断是否存在重名文件
 * @param {string} filePath - 文件的完整路径，包含文件名和扩展名
 */
declare function load(filePath: string): void;
declare function beforeBuildEnd(): void;
export {};
