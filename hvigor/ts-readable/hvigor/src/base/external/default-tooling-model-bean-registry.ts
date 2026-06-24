/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/default-tooling-model-bean-registry.js
 */
import * as hvigor_log_js_1 from "../log/hvigor-log";
class DefaultToolingModelBeanRegistry {
  constructor() {
    this._log = hvigor_log_js_1.HvigorLogger.getLogger("ToolingModelBeanRegistry"), this._modelMap = new Map();
  }
  registry(e) {
    const o = e.modelId;
    this._modelMap.has(o) && this._log.error("Multiple models have registered."), this._modelMap.set(o, e);
  }
  getModelMap() {
    return this._modelMap;
  }
  clear() {
    this._modelMap.clear();
  }
}
export const defaultModelRegistry = new DefaultToolingModelBeanRegistry();
