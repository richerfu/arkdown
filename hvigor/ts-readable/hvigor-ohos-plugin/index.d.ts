/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/index.d.ts
 */
import { Module, Project } from '@ohos/hvigor';
/**
 * Stage模型的hap插件
 *
 * @param module hvigorModule
 */
export declare const hapTasks: (module: Module) => import("./src/plugin/common/abstract-hap-module-plugin").AbstractHapModulePlugin;
/**
 * Fa模型的hap插件
 *
 * @param module hvigorModule
 */
export declare const legacyHapTasks: (module: Module) => import("./src/plugin/common/abstract-hap-module-plugin").AbstractHapModulePlugin;
/**
 * Stage模型的app插件
 *
 * @param module hvigorProject
 */
export declare const appTasks: (module: Project) => import("./src/plugin/app-plugin").AppPlugin;
/**
 * Fa模型的app插件
 *
 * @param module hvigorProject
 */
export declare const legacyAppTasks: (module: Project) => import("./src/plugin/app-plugin").AppPlugin;
/**
 * Stage模型的Har插件
 *
 * @param module hvigorModule
 */
export declare const harTasks: (module: Module) => import("./src/plugin/common/abstract-har-module-plugin").AbstractHarModulePlugin;
/**
 * Fa模型的Har插件
 *
 * @param module hvigorModule
 */
export declare const legacyHarTasks: (module: Module) => import("./src/plugin/common/abstract-har-module-plugin").AbstractHarModulePlugin;
/**
 * Stage模型的hsp插件，注意没有Fa模型的hsp插件
 *
 * @param module hvigorModule
 */
export declare const hspTasks: (module: Module) => import("./src/plugin/hsp-plugin").HspPlugin;
export { OhosAppContext, OhosHapContext, OhosHarContext, OhosHspContext, OhosPluginId, OhpmDependencyInfo, Product, Target, } from './src/external/api/ohos-plugin-api';
