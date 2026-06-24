/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/libs-file-cache-util.d.ts
 */
/// <reference types="node" />
import { Stats } from 'fs';
import { DebugSymbol } from '../options/build/build-opt';
import { Option } from '../options/options';
import { BinxoOptions } from '../tasks/worker/native-binxo';
import { StripOptions } from '../tasks/worker/native-strip';
export declare class LibsFileCacheUtil {
    static getFileHash(stats: Stats): string;
    /**
     * 根据一个文件的路径和cacheJson判断是否被改变过(用与判断processLibs目录中的文件)
     *
     * @param stripOptions strip的具体配置
     * @param fileAbsolutePath 文件的绝对路径
     * @param fileCacheJsonObj cacheJson对象 通过本类的readFileCacheJsonObj方法获取
     * @param intermediatesProcessLibs 文件存在的模块的processLib路径
     * @param debugSymbol 本次构建的strip选项
     * @return true 被修改过 false 没有被修改
     */
    static isSourceChanged(stripOptions: StripOptions, fileAbsolutePath: string, fileCacheJsonObj: NativeLibsCache, intermediatesProcessLibs: string, debugSymbol: DebugSymbol | undefined): Promise<boolean>;
    /**
     * 根据一个文件的路径和cacheJson判断是否被改变过(用与判断processLibs目录中的文件)
     *
     * @param binxoOptions binxo的具体配置
     * @param fileAbsolutePath ProcessLibs文件中一个so的绝对路径，例如：D:xxx\MyApplicationbinxo1\entry\libs\arm64-v8a\xxx.so
     * @param fileCacheJsonObj cacheJson对象 通过本类的readFileCacheJsonObj方法获取
     * @param intermediatesProcessLibs 文件存在的模块的processLib路径,例如:D:xxx\MyApplicationbinxo1\entry\libs
     * @return true 被修改过 false 没有被修改
     */
    static isBinxoSourceChanged(binxoOptions: BinxoOptions, fileAbsolutePath: string, fileCacheJsonObj: NativeLibsBinxoCache, intermediatesProcessLibs: string): Promise<boolean>;
    private static isStripped;
    /**
     * 通过filesExcluding拿到要进行Binxo的so集合，判断传入的so上次是否已经被Binxo
     * @param binxoOptions
     * @param binxoSymbol
     * @param fileRelativePath
     * @param processLibs
     * @return true,上一次so被Binxo
     */
    private static isBinxoed;
    /**
     * 根据一个文件的路径和cacheJson判断是否被改变过(用于判断strippedProcessLibs目录中的文件)
     *
     * @param fileAbsolutePath 文件的绝对路径
     * @param libsFileCacheJsonObj cacheJson对象 通过本类的readFileCacheJsonObj方法获取
     * @return true 被修改过 false 没有被修改
     */
    static isSinkChanged(fileAbsolutePath: string, libsFileCacheJsonObj: NativeLibsCache): Promise<boolean>;
    /**
     * 根据一个文件的路径和cacheJson判断是否被改变过(用于判断binxo目录中的文件)
     *
     * @param fileAbsolutePath 文件的绝对路径
     * @param libsFileCacheJsonObj cacheJson对象 通过本类的readFileCacheJsonObj方法获取
     * @return true 被修改过 false 没有被修改
     */
    static isBinxoLibsChanged(fileAbsolutePath: string, libsFileCacheJsonObj: NativeLibsBinxoCache): Promise<boolean>;
    /**
     * 根据一个绝对路径 生成这个绝对路径下所有文件的hash值并返回(递归处理子文件夹)
     *
     * @param absolutePath 绝对路径
     * @return hash值的json字符串
     */
    static generateFileHashesForDirectory(absolutePath: string): Promise<Record<string, string>>;
    /**
     * 刷新libs file的缓存
     *
     * @param originalLibs
     * @param strippedLibs
     * @param debugSymbol 用户填写的debugSymbol选项
     * @param cacheFilePath 缓存文件的路径
     */
    static refreshLibsFileCache(originalLibs: string, strippedLibs: string, cacheFilePath: string, debugSymbol?: DebugSymbol): Promise<void>;
    /**
     * 刷新libs file的缓存
     *
     * @param originalLibs
     * @param binxoLibs
     * @param binxoSymbol 用户填写的binxoSymbol选项
     * @param cacheFilePath 缓存文件的路径
     */
    static refreshBinxoFileCache(originalLibs: string, binxoLibs: string, cacheFilePath: string, binxoSymbol?: BinxoSymbol): Promise<void>;
}
export interface NativeLibsCache {
    libs: Record<string, string>;
    stripped: Record<string, string>;
    debugSymbol: DebugSymbol | undefined;
}
export interface BinxoSymbol extends Option {
    enableAsanBinxo: boolean;
    excludeSoFromBinxo: string[];
}
export interface NativeLibsBinxoCache {
    libs: Record<string, string>;
    binxo: Record<string, string>;
    binxoSymbol: BinxoSymbol | undefined;
}
