/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/build-option-path-info.d.ts
 */
import { CoreModuleModelImpl } from '../model/module/core-module-model-impl';
import { ProjectModel } from '../model/project/project-model';
import { AppJson } from '../options/configure/app-json-options';
declare class BuildOptionPath {
    private productOptPath;
    private buildOptionPath;
    private signConfigPath;
    private appConfigPath;
    setAppConfigPath(appJsonPath: string, appJsonOpt: AppJson.AppOptObj, srcPath: string): void;
    initAppConfigPath(appJsonPath: string): void;
    getAppConfigPath(appJson5Path: string, appConfigName: string): string;
    setProductOptPath(newMap: Map<string, Attributes>, product: string): void;
    setBuildOptionPath(moduleName: string, target: string, newMap: Map<string, Attributes>): void;
    /**
     * 通过 product.Name 来获取对应的product buildOpt
     * @param productName
     */
    getProductOptPath(productName: string): Map<string, Attributes> | undefined;
    /**
     *  获取到签名配置的路径来源
     * @param project
     * @param attributeName
     */
    getSignConfigPath(project: ProjectModel, attributeName: string): string;
    initSignConfig(project: ProjectModel, path: string): void;
    /**
     * 获取对应字段的配置路径
     * @param moduleModel module
     * @param targetName target Name
     * @param attributesName 属性名称
     */
    getTargetBuildOptPath(moduleModel: CoreModuleModelImpl, targetName: string, attributesName: string): string;
}
/**
 * 合并map 与 newMap，且以map为基准
 * 判断两个map里面存储的属性的值是否改变，如果改变就将newMap中的值和路径填到map中
 * 如果是模块级别的合并，两个map的值相等也优先存入路径中包含moduleName的路径
 *
 * @param map 原有的path Map
 * @param newMap 新的path Map
 * @param moduleName
 */
export declare function mergePath(map: Map<string, Attributes>, newMap: Map<string, Attributes>, moduleName?: string): Map<string, Attributes>;
export declare const buildOptionPath: BuildOptionPath;
export interface Attributes {
    value: string;
    srcPath: string;
}
/**
 * 通过 new Error的方法获取堆栈来找出调用hook函数的文件
 * 目前仅适配 setBuildProfileOpt() 方法
 */
export declare function getHookFilePath(): string;
export {};
