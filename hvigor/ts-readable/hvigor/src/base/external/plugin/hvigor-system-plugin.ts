/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/plugin/hvigor-system-plugin.js
 */
import * as const_js_1 from "../../../common/const/const";
export class HvigorSystemPlugin {
  constructor(s) {
    this.pluginVersion = const_js_1.CUR_HVIGOR_VERSION, this.pluginId = s;
  }
  getPluginId() {
    return this.pluginId;
  }
}
