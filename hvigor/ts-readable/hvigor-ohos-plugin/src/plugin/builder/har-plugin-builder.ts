/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/builder/har-plugin-builder.js
 */
import * as abstract_plugin_builder_js_1 from "./abstract-plugin-builder";
import * as hvigor_1 from "@ohos/hvigor";
import * as plugin_inspector_js_1 from "../factory/plugin-inspector";
export class HarPluginBuilder extends abstract_plugin_builder_js_1.AbstractPluginBuilder {
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
    return this._log.debug("Har plugin creating..."), null === (e = this.initializationStrategy) || undefined === e ? undefined : e.createHarPlugin(this.module, this);
  }
  registerHooks(e) {
    this._log.debug("Har plugin register hook start..."), hvigor_1.hvigorCore.hvigorAfterNodeInternalHook(this.module.getName(), async () => {
      e.initContext();
    }), hvigor_1.hvigorCore.hvigorNodeEvaluatedInternalHook(() => {
      var r;
      this.ifThrowBuildModeError(), null === (r = e.getTaskService()) || undefined === r || r.initDependencyInfo(), plugin_inspector_js_1.PluginInspector.checkAtomicServiceHspModules(e), this.initializeTaskDepends(this.module, this.creatorManager, this.isFaMode);
    }), this._log.debug("Har plugin register hook end...");
  }
}
