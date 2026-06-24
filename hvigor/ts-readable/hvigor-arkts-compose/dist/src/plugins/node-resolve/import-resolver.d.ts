/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/import-resolver.d.ts
 */
import { DependencyInfoType } from '../../project-config/build-option/build-option';
import { GetCachedStat } from '../../util/path-cache-stat';
interface ResolvedImportInfo {
    packageName: string;
    normalizedPath: string;
}
export declare class ImportResolver {
    private depName2DepInfos;
    private caseSensitiveCheck;
    private getInfo;
    constructor(depName2DepInfos: Map<string, DependencyInfoType>, caseSensitiveCheck: boolean, getInfo?: GetCachedStat);
    /**
     * 提供给OH的接口，解析hsp和byteCodeHar的importedId为标准路径，处理importedId中有多余斜杠的各种场景
     * 如果importedId不是hsp或byteCodeHar的，返回undefined；如果不能正确解析出路径则返回undefined。
     * 在一些错误的importedId场景下，在编译阶段就会报错，走不到OH调用此接口。比如importedId是一个源码ets文件，
     * importedId为源码文件相对路径，后缀为.ets，则在编译阶段就报错不支持.ets后缀
     * @param referenceName
     */
    resolve: (referenceName: string) => ResolvedImportInfo | undefined;
    /**
     * 去掉文件路径的文件名后缀，返回不带文件后缀的文件路径
     * @param filePath
     */
    private removeFileExtensionFromPath;
    /**
     * 判断filePath是否是文件目录
     * @param filePath
     */
    private pathIsDirectory;
    /**
     * 判断路径是否是文件路径
     * @param filePath
     */
    private pathIsFile;
    private pathIsFilePath;
    /**
     * 获取importee的绝对路径：先替换掉importee最前面的packageName（同时避免了packageName和模块物理路径不一致的问题），然后拼接pkgRootPath
     *
     * @param pkgRootPath
     * @param referenceName
     * @param dependencyName
     * @private
     */
    private getReferenceNamePath;
    /**
     * 得到解析路径
     * @param isPkgNameMatchInCase 包名是否大小写一致
     * @param regex 匹配包名的正则表达式
     * @param referencePath 使用path.normalize标准化后的importee
     * @param referenceNamePath importee对应的绝对路径
     * @param referenceName importee
     * @private
     */
    private getResReferencePath;
}
export {};
