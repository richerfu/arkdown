/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har-deduplicate/deduplicate-util.d.ts
 */
import { DuplicatedHarDependency, PkgContextInfoOpt } from '@ohos/hvigor-arkts-compose';
import { DynamicImportObj } from '../generate-loader-json';
/**
 * 过滤要去重的har包分别得到字节码har,源码har,和非so源码har
 * @param hspNeedDeduplicateHarInfoMap
 */
export declare function getFilteredDuplicateHarInfo(hspNeedDeduplicateHarInfoMap: Record<string, DuplicatedHarDependency>): {
    byteCodeHarPkgNameSet: Set<string>;
    sourceHarPkgPathSet: Set<string>;
    sourceHarWithoutSoPkgPathSet: Set<string>;
};
/**
 * declarationEntry 的合并
 */
export declare function mergeDeclarationEntry(src: string[] | Set<string>, target: string[]): void;
/**
 * compileEntry 的合并
 */
export declare function mergeCompileEntry(src: string[], target: string[]): void;
/**
 * dynamicImportLibInfo 的合并
 */
export declare function mergeDynamicImportLibInfo(src: Record<string, object>, target: Record<string, object>): void;
/**
 * workers 的合并
 */
export declare function mergeWorkers(src: string[], target: string[]): void;
/**
 * hsp编译时搜集到的字节码har的引用（可能是包名引用或者包名+路径引用）
 * 这些搜集到的字节码har引用已经确定要打入到entry中了，但entry有可能并没有真正引用到这些字节码har,
 * 所以需要把这些引用加到declarationEntry中防止被sdk因为未使用到而被裁剪掉。
 */
export declare function processByteCodeHarDeclarationEntry(byteCodeHarName: string, byteCodeHarImportNames: string[], pkgContextInfo: PkgContextInfoOpt, hasProcessedImportNameSet: Set<string>, needAddDeclarationEntrySet: Set<string>): void;
/**
 *  worker去重
 *  hsp依赖的源码har的worker会被加入到workers字段中作为编译入口，如果har被去重，这些worker要从hsp移除
 * @param currentWorkerConfig 当前的 worker config
 * @param sourceHarWithoutSoPkgPathSet 要去重的源码har路径
 */
export declare function deduplicateWorkers(currentWorkerConfig: string[], sourceHarWithoutSoPkgPathSet: Set<string>): string[];
/**
 * compileEntry 去重
 * hsp依赖的源码har的worker和动态import会作为入口文件添加到compileEntry中，如果har被去重，这些要从hsp中移除
 * @param currentCompileEntry compileEntry
 * @param sourceHarWithoutSoPkgPathSet 非so源码har
 */
export declare function deduplicateCompileEntry(currentCompileEntry: string[], sourceHarWithoutSoPkgPathSet: Set<string>): string[];
/**
 * otherCompileFiles 去重
 * 要去重的字节码har直接依赖的源码HAR或者NPM三方包的代码需要从otherCompileFiles移除，同时之后添加到hap的otherCompileFiles中。
 * @param curOtherCompileFile curOtherCompileFile
 * @param sourceHarWithoutSoPkgPathSet 非so源码har
 */
export declare function deduplicateOtherCompileFiles(curOtherCompileFile: string[], sourceHarWithoutSoPkgPathSet: Set<string>): string[];
/**
 * declarationEntry去重
 * 要去重的字节码har的declarationEntry要移除，同时之后添加到hap的declarationEntry中。
 * 字节码har里的worker文件和runtimeOnly.sources也在declarationEntry中，这2个也要合并到entry的declarationEntry中防止被裁剪。
 * @param harName 字节码har pkgName
 * @param currentDeclarationEntry 当前的 declarationEntry
 */
export declare function deduplicateDeclarationEntry(harName: string, currentDeclarationEntry: string[]): string[];
/**
 * dynamicImportLibInfo 去重
 * 要去重的har如果在 dynamicImportLibInfo 中，这些信息也要从hsp中移除，之后合并到hap中。
 * @param currentDynamicImportLibInfo 当前 dynamicImportLibInfo
 * @param sourceHarPkgPathSet 源码har路径集合
 */
export declare function deduplicateDynamicImportLibInfoObj(currentDynamicImportLibInfo: Record<string, DynamicImportObj>, sourceHarPkgPathSet: Set<string>): Record<string, DynamicImportObj>;
/**
 *根据条件从原数组中删除一项，同时搜集返回删除元素的数组
 *
 *@param originArray 原数组
 *@param predicate 筛选条件,参数为单个元素
 */
export declare function deduplicateArray<T>(originArray: T[], predicate: (item: T) => boolean): T[];
/**
 * 合并src数组到target数组（如果target数组中根据predicate条件找不到对应元素）
 *
 * @param srcArray 原数组
 * @param targetArray 目标数组
 * @param predicate 是否存在比较条件
 * @param callback 回调，参数添加的元素
 */
export declare function mergeTwoArray<T>(srcArray: T[], targetArray: T[], predicate: (srcItem: T, targetItem: T) => boolean, callback?: (item: T) => void): void;
