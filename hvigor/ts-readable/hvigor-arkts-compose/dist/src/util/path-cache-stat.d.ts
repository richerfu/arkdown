/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/path-cache-stat.d.ts
 */
interface FileInfo {
    exists: boolean;
    isDirectory: boolean;
    isFile: boolean;
    mtimeMs: number;
    size: number;
}
export type GetCachedStat = (filePath: string) => FileInfo;
/**
 * 使用rollup的GetCachedFileInfo接口获取文件、目录信息工具类
 * 使用场景：适用于获取rollup过程中不会变动的文件或者目录的信息。通过缓存机制提高获取文件或者目录信息的性能
 */
export declare class PathCacheStat {
    /**
     * 判断路径是否是目录
     * @param fileDirPath 文件或者目录路径
     * @param getInfo 函数，用于从缓存中获取信息
     */
    static isDirectory(fileDirPath: string, getInfo?: GetCachedStat): boolean;
    /**
     * 判断路径是否是文件
     * @param fileDirPath 文件或者目录路径
     * @param getInfo 函数，用于从缓存中获取信息
     */
    static isFile(fileDirPath: string, getInfo?: GetCachedStat): boolean;
    /**
     * 判断路径是否存在
     * @param fileDirPath 文件或者目录路径
     * @param getInfo 函数，用于从缓存中获取信息
     */
    static exists(fileDirPath: string, getInfo?: GetCachedStat): boolean;
    /**
     * 获取文件或者目录路径的信息
     * @param fileDirPath 文件或者目录路径
     * @param getInfo 函数，用于从缓存中获取信息
     */
    static getStat(fileDirPath: string, getInfo?: GetCachedStat): FileInfo;
}
export {};
