/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/plugin/default-plugin-container.js
 */
import * as hvigor_log_js_1 from "../../log/hvigor-log";
const log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-plugin-container");
export class DefaultPluginContainer {
  constructor() {
    this._pluginMap = new Map();
  }
  registryPlugin(t) {
    const e = t.getPluginId();
    return this._pluginMap.has(e) && log.printErrorExit("REGISTRY_PLUGIN_ERROR", [e]), this._pluginMap.set(e, t), t;
  }
  getPluginById(t) {
    return this._pluginMap.get(t);
  }
}
