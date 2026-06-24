/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/path-util.d.ts
 */
export declare function isSubPath(subPath: string, basePath: string): boolean;
/**
 * 判断subPath是否为basePath的子路径,subPath对应目录是basePath目录的子目录
 * 调用方保证subPath和basePath都是经过normalized后的绝对路径，subPath===basePath时返回true
 */
export declare function isChildPath(subPath: string, basePath: string): boolean;
