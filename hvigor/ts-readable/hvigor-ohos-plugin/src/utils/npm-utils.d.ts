/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/npm-utils.d.ts
 */
import { DurationEvent } from '@ohos/hvigor';
import { OhPackageJsonOpt, PackageJsonOpt } from '../tasks/task/ohos-har-task';
import { OhosLogger } from './log/ohos-logger';
/**
 * 对某个package.json文件检查其是否具有本地依赖
 * @param {string} packagePath 包路径
 * @param packageJsonObj 包解析后的对象
 * @param option 定制检查的配置
 *  - allowInside允许跳过在本目录内的本地依赖
 * @returns {boolean}
 */
export declare function checkHasLocalFileDependency(packagePath: string, packageJsonObj: PackageJsonOpt | OhPackageJsonOpt, option?: {
    allowInside?: boolean;
}): boolean;
/**
 * 对某个包收集在包内的所有本地依赖
 * @param {string} packagePath 包路径
 * @param packageJsonObj  包解析后的对象
 * @returns {string[]}
 */
export declare function collectLocalFileDependency(packagePath: string, packageJsonObj: PackageJsonOpt | OhPackageJsonOpt): string[];
/**
 * 判断某条路径是否为本地依赖
 * @param {string | undefined} spec
 * @returns {boolean}
 */
export declare function isLocalDependency(spec: string | undefined): boolean;
/**
 * 返回npm路径
 * @returns {string}
 */
export declare function getNpmPath(): string;
/**
 * 使用tar算法对sourceDir打包, 打包产物保存在destHarPath
 *
 * @param sourceDir 待打包路径
 * @param destHarPath 打包产物路径
 * @param durationEvent 持续事件
 * @param needPrefix 是否使用package包一层
 */
export declare function execOhpmPack(sourceDir: string, destHarPath: string, durationEvent: DurationEvent, needPrefix?: boolean): Promise<void>;
/**
 * 根据package.json的main字段，拼接出对应的types字段字符串
 *
 * @param {string | undefined} main main字段的值
 * @returns {string} types字段的值
 * @private
 */
export declare const getTypesFieldFromMainField: (main: string | undefined) => string;
/**
 * 仅用于本地依赖参数化场景，在 parameterFile 中填写了本地产物依赖(har/hsp)时需要将依赖打进 har 包中
 * 假如产物依赖在本模块外部，需要打进 har 包的根目录
 * 假如产物依赖在本模块内部，需要保持原有的模块相对路径打进 har 包中
 *
 * @param params
 */
export declare function copyParameterizationLocalProductDependencies(params: {
    localProductDependencies: ReadonlyArray<string>;
    moduleDir: string;
    /** 将要压缩成 har 包的临时目录 */
    tempDir: string;
    logger: OhosLogger;
}): void;
