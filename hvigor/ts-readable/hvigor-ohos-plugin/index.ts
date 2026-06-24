/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/index.js
 */
import * as plugin_factory_js_1 from "./src/plugin/factory/plugin-factory";
import * as remote_hsp_utils_js_1 from "./src/utils/remote-hsp-utils";
import * as command_builder_modifier_js_1 from "./src/builder/command-builder-modifier";
import * as ohos_plugin_api_js_1 from "./src/external/api/ohos-plugin-api";
export const hapTasks = s => plugin_factory_js_1.PluginFactory.getHapPlugin(s);
export const legacyHapTasks = s => plugin_factory_js_1.PluginFactory.getHapPlugin(s, true);
export const appTasks = s => plugin_factory_js_1.PluginFactory.getAppPlugin(s);
export const legacyAppTasks = s => plugin_factory_js_1.PluginFactory.getAppPlugin(s, true);
export const harTasks = s => plugin_factory_js_1.PluginFactory.getHarPlugin(s);
export const legacyHarTasks = s => plugin_factory_js_1.PluginFactory.getHarPlugin(s, true);
export const hspTasks = s => plugin_factory_js_1.PluginFactory.getHspPlugin(s);
export { CommandBuilderType, OhosPluginId, getRemoteHspObjList, registryCommandModifier, getRemoteHspObjList, CommandBuilderType, registryCommandModifier, OhosPluginId };
