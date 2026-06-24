/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/module-option-path-info.d.ts
 */
import { ModuleJson } from '../options/configure/module-json-options';
import ModuleObj = ModuleJson.ModuleObj;
declare class ModuleOptionPathInfo {
    private moduleOptPathMap;
    initModuleOptPathMap(moduleJson5Path: string): void;
    setModuleOptPathMap(moduleJson5Path: string, moduleOpt: ModuleObj, srcPath: string): void;
    getModuleOptPath(moduleJson5Path: string, optName: string): string;
}
export declare const moduleOptionPathInfo: ModuleOptionPathInfo;
export {};
