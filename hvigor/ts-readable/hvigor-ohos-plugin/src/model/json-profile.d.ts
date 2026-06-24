/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/json-profile.d.ts
 */
import { ModuleJson } from '../options/configure/module-json-options';
import ModuleOptObj = ModuleJson.ModuleOptObj;
import { ConfigJson } from '../options/configure/config-json-options';
import ConfigOptObj = ConfigJson.ConfigOptObj;
/**
 * 根据不同模型获取module.json5/config.json相关数据
 */
export interface JsonProfile {
    jsonFilePath: string;
    profile: ModuleOptObj | ConfigOptObj;
    deviceTypes: string[];
    deviceConfig: string;
    configurationProfile: string;
}
