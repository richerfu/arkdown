/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/res-model-loader.d.ts
 */
import { AppJson } from '../../../options/configure/app-json-options';
import { ConfigJson } from '../../../options/configure/config-json-options';
import { ModuleJson } from '../../../options/configure/module-json-options';
declare class ResModelLoader {
    private readonly _log;
    private appJsonSchemaPath;
    private moduleJsonSchemaPath;
    getAppJson(appJsonPath: string): AppJson.AppOptObj;
    getModuleJson(moduleJsonPath: string): ModuleJson.ModuleOptObj;
    getConfigJson(configJsonPath: string): ConfigJson.ConfigOptObj;
    setAppJson(appJsonPath: string, appJsonObj: AppJson.AppOptObj): void;
    setModuleJson(moduleJsonPath: string, moduleJsonObj: ModuleJson.ModuleOptObj): void;
    /**
     * 从configFileLoader中读取一个配置文件，如果不存在则会从文件中读取
     * @param jsonPath
     * @param schemaPath 校验的schema文件路径
     * @private
     */
    private getResModelJsonObj;
    private setResModelJsonObj;
    protected validateBuildProfile(obj: object, schemaPath: string): void;
    initSchemaPath(appJsonSchemaPath: string, moduleJsonSchemaPath: string): void;
    clean(): void;
}
export declare const resModelLoader: ResModelLoader;
export {};
