/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/plugin-factory.js
 */
import * as app_plugin_builder_js_1 from "../builder/app-plugin-builder";
import * as hap_plugin_builder_js_1 from "../builder/hap-plugin-builder";
import * as har_plugin_builder_js_1 from "../builder/har-plugin-builder";
import * as hsp_plugin_builder_js_1 from "../builder/hsp-plugin-builder";
export class PluginFactory {
  static getAppPlugin(i, u = false) {
    return new app_plugin_builder_js_1.AppPluginBuilder(i, u).build();
  }
  static getHapPlugin(i, u = false) {
    return new hap_plugin_builder_js_1.HapPluginBuilder(i, u).build();
  }
  static getHarPlugin(i, u = false) {
    return new har_plugin_builder_js_1.HarPluginBuilder(i, u).build();
  }
  static getHspPlugin(i) {
    return new hsp_plugin_builder_js_1.HspPluginBuilder(i).build();
  }
}
