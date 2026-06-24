/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/builder/hsp-plugin-builder.js
 */
import * as abstract_plugin_builder_js_1 from "./abstract-plugin-builder";
import * as hvigor_1 from "@ohos/hvigor";
import * as module_ohos_config_manager_js_1 from "../../common/global/module-ohos-config-manager";
export class HspPluginBuilder extends abstract_plugin_builder_js_1.AbstractPluginBuilder {
  constructor(e, r = false) {
    super(r), this.module = e;
  }
  get creatorManager() {
    return this._creatorManager;
  }
  set creatorManager(e) {
    this._creatorManager = e;
  }
  createPlugin() {
    var e;
    return this._log.debug("Hsp plugin creating..."), null === (e = this.initializationStrategy) || undefined === e ? undefined : e.createHspPlugin(this.module, this);
  }
  registerHooks(e) {
    this._log.debug("Hsp plugin register hook start..."), hvigor_1.hvigorCore.hvigorAfterNodeInternalHook(this.module.getName(), async () => {
      e.initContext();
    }), hvigor_1.hvigorCore.hvigorNodeEvaluatedInternalHook(() => {
      var r;
      this.ifThrowBuildModeError(), null === (r = e.getTaskService()) || undefined === r || r.initDependencyInfo(), this.creatorManager.configure();
    }), this._log.debug("Hsp plugin register hook end...");
  }
  initHspPluginGlobalData(e) {
    var r;
    const o = null === (r = e.getConfigOpt()) || undefined === r ? undefined : r.getObject("ohos");
    o && module_ohos_config_manager_js_1.moduleOhosConfigManager.loaderConfig(e.getName(), o);
  }
}
