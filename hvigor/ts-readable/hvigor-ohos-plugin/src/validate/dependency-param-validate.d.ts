/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/validate/dependency-param-validate.d.ts
 */
import { pathParamsOpt } from './dependency-tag-validate';
interface NestedObject {
    [key: string]: any;
}
export interface ParameterizationMatchParams {
    /** 假如在 oh-package.json 中填写了 @param:key1.key2.key3，则该参数为 ['key1','key2','key3'] */
    keys: string[];
    /** parameterFile.json5 对象 */
    parameterFileJson: Record<string, unknown>;
    /** oh-package.json5 和 parameterFile.json5 的文件路径 */
    pathParamsObj: pathParamsOpt;
    /**
     * 当读取 parameterFile 中的属性值是本地依赖而不是版本号时，需要去获取其他模块的 oh-package.json 信息
     * 假如不传这个参数则不会替换参数化的本地依赖
     */
    ohPackageInfoGetter?: OhPackageInfoGetter;
    /**
     * 假如在 oh-package.json5 中填写了 har1: '@param:dep.har1'，则该参数为 har1
     * 用于检查依赖名与被依赖模块的name是否相同，即 har1 的 oh-package.json5 name 必须也是 har1
     * 也用于检查 tag 不可用于 version 字段，详看错误码 HE10003
     */
    dependencyName?: string;
}
export interface ParameterizationMatchResult {
    value: string;
    localDepArr?: string[];
}
/**
 * 当读取 parameterFile 中的属性值是模块目录时，需要去获取其他模块的 oh-package.json 信息
 */
export type OhPackageInfoGetter = (ohPackageJsonAbsolutePath: string) => {
    version: string;
    name: string;
};
/**
 * 假如在 oh-package.json5 中填写了 @param:key1.key2.key3，该函数根据 key1.key2.key3 在 parameterFile.json5 中查找相应的属性值
 * 注意 . 可能会是键的一部分，在 parameterFile.json5 可以有 'key1.key2.key3': '1.0.0'，优先级详看单测 dependency-param-validate.unittest.ts
 * @param params
 */
export declare function matchValueWithParameterizationKey({ keys, parameterFileJson, pathParamsObj, ohPackageInfoGetter, dependencyName, }: ParameterizationMatchParams): ParameterizationMatchResult;
/**
 * 校验parameterFile的key值是否符合规格
 *
 * @param parameterFileObj
 */
export declare function validateParameterKeys(parameterFileObj: NestedObject): void;
export {};
