/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/plugin/interface/plugin-context.js
 */
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
export class PluginContext {
  constructor() {
    this.logger = hvigor_log_js_1.HvigorLogger.getLogger(PluginContext.name), this.contextMap = new Map();
  }
  setContextFunc(t, e) {
    this.contextMap.set(t, e);
  }
  getContext(t) {
    const e = this.contextMap.get(t);
    if (this.contextMap.has(t) && "function" == typeof e) return e();
  }
}
