/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/res/res-model.d.ts
 */
import { AppJson } from '../../options/configure/app-json-options';
import { ConfigJson } from '../../options/configure/config-json-options';
import { ModuleJson } from '../../options/configure/module-json-options';
import AppOptObj = AppJson.AppOptObj;
import ModuleOptObj = ModuleJson.ModuleOptObj;
import ConfigOptObj = ConfigJson.ConfigOptObj;
import FormsObj = ModuleJson.FormsObj;
/**
 * 资源和工程配置文件的模型
 */
export interface ResModel {
    /**
     * 获取json文件的路径
     */
    getJsonPath(): string;
    /**
     * 获取resource目录的路径
     */
    getResourcePath(): string;
    /**
     * 获取json文件的内容
     */
    getJsonContent(): string;
}
export interface AppRes extends ResModel {
    /**
     * 获取app.json5的对象
     */
    getAppResOpt(): AppOptObj;
}
export interface ModuleTargetRes extends ResModel {
    /**
     * 获取target下module.json5的对象
     */
    getModuleJsonOpt(): ModuleOptObj;
    /**
     * 获取target下form_config.json5的对象
     */
    getFormJsonOpt(formPath: string): FormsObj;
}
export interface LegacyModuleTargetRes extends ResModel {
    /**
     * 获取target下config.json的对象
     */
    getConfigJsonOpt(): ConfigOptObj;
    supportLiteDeviceModule(): boolean;
}
