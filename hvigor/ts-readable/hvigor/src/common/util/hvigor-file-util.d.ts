/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/hvigor-file-util.d.ts
 */
/// <reference types="node" />
/// <reference types="node" />
/**
 * 自定义的File类
 */
export declare class NormalizedFile {
    readonly filePath: string;
    private logger;
    constructor(_path: string);
    /**
     * 获取到NormalizedFile对象下深层递归的目录与文件NormalizedFile[]，包含它本身
     *
     *@return NormalizedFile[]
     */
    asFileList(): NormalizedFile[];
    private _asFileList;
    /**
     * 支持在原有的目录下进行链式拼接路径
     *
     * @param _path
     * @return NormalizedFile
     */
    file(_path: string): NormalizedFile | undefined;
    private _file;
    /**
     * 获取当前文件/目录的路径
     *
     * @return string
     */
    getPath(): string;
    private _getPath;
}
/**
 * 封装的FileUtil工具类
 */
export declare class FileUtil {
    private static logger;
    /**
     * 判断文件/目录是否存在
     *
     * @param filePath {string} 文件/目录的地址字符串
     * @return boolean
     */
    static exist(filePath: string): boolean;
    private static _exist;
    /**
     * 判断是否是目录
     *
     * @param file {string | NormalizedFile} 文件的路径或者是NormalizedFile
     * @return boolean
     */
    static isDictionary(file: string | NormalizedFile): boolean;
    private static _isDictionary;
    /**
     * 判断是否是文件
     *
     * @param file {string | NormalizedFile} 文件的路径或者是NormalizedFile
     * @return boolean
     */
    static isFile(file: string | NormalizedFile): boolean;
    private static _isFile;
    /**
     * 确保目录存在，不存在就创建
     *
     * @param dirPath {string} 目录路径
     */
    static ensureDirSync(dirPath: string): void;
    private static _ensureDirSync;
    /**
     * 确保文件存在 不存在就创建
     *
     * @param filePath {string} 文件路径
     */
    static ensureFileSync(filePath: string): void;
    private static _ensureFileSync;
    /**
     * 同步读取文件
     *
     * @param file {string | NormalizedFile}
     * @return {Buffer}
     */
    static readFileSync(file: string | NormalizedFile): Buffer;
    private static _readFileSync;
    /**
     * 异步读取文件
     *
     * @param file {string | NormalizedFile}
     * @return {undefined | Promise<Buffer>}
     */
    static readFile(file: string | NormalizedFile): Promise<Buffer> | undefined;
    private static _readFile;
    /**
     * 同步读取Json5文件
     *
     * @param file {string | NormalizedFile}
     * @return {JSON}
     */
    static readJson5(file: string | NormalizedFile): any;
    private static _readJson5;
    /**
     * 异步写入文件
     *
     * @param file {string | NormalizedFile} 文件路径或者NormalizedFile 对象
     * @param content 写入内容
     */
    static writeFile(file: string | NormalizedFile, content: any): Promise<void>;
    private static _writeFile;
    /**
     * 同步写入文件
     *
     * @param file {string | NormalizedFile} 文件路径或者NormalizedFile 对象
     * @param content 写入内容
     */
    static writeFileSync(file: string | NormalizedFile, content: any): void;
    private static _writeFileSync;
    /**
     * 异步复制文件
     *
     * @param file {string | NormalizedFile} 文件路径或者NormalizedFile对象
     * @param dest {string} 目标文件地址
     */
    static copyFile(file: string | NormalizedFile, dest: string): Promise<void>;
    private static _copyFile;
    /**
     * 同步复制文件
     *
     * @param file {string | NormalizedFile} 文件路径或者NormalizedFile对象
     * @param dest {string} 目标文件地址
     */
    static copyFileSync(file: string | NormalizedFile, dest: string): void;
    private static _copyFileSync;
    /**
     * 拼接路径方法类
     *
     * @param paths
     * @return string 拼接好的路径
     */
    static pathResolve(...paths: string[]): string;
    private static _pathResolve;
}
