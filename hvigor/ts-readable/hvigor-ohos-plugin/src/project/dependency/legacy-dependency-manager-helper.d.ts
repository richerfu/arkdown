/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/legacy-dependency-manager-helper.d.ts
 */
import { OhosPackageJson } from '../../utils/dependency-util';
/**
 * 为FA模型支持管理har的依赖
 */
export declare class LegacyDependencyManagerHelper {
    private readonly _moduleName;
    constructor(moduleName: string);
    /**
     * 过config.json/module.json5判断当前模块是否与FA模型兼容
     * @param pkgPath 当前模块的路径
     * @param devPkgJsonObj
     */
    checkHspOrHarStatusForFAByProfile(pkgPath: string, devPkgJsonObj: OhosPackageJson): boolean;
    /**
     * 判断依赖的包与本FA模块是否兼容, 需要传入module.json或module.json5路径
     * @param runtimeJson 传入module.json或module.json5路径
     * @param devPkgJsonObj
     */
    checkHspOrHarStatusForFA(runtimeJson: string, devPkgJsonObj: OhosPackageJson): boolean;
}
