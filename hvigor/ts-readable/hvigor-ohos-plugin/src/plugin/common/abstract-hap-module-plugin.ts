/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/common/abstract-hap-module-plugin.js
 */
import * as module_ohos_config_manager_js_1 from "../../common/global/module-ohos-config-manager";
import * as abstract_module_plugin_js_1 from "./abstract-module-plugin";
import * as ohos_plugin_id_js_1 from "./ohos-plugin-id";
export class AbstractHapModulePlugin extends abstract_module_plugin_js_1.AbstractModulePlugin {
  constructor(o) {
    super(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN, o);
  }
  initGlobalData() {
    var o;
    const e = null === (o = this._module.getConfigOpt()) || undefined === o ? undefined : o.getObject("ohos");
    e && module_ohos_config_manager_js_1.moduleOhosConfigManager.loaderConfig(this.moduleName, e);
  }
}
