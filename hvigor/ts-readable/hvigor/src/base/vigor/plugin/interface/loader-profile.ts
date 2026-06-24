/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/plugin/interface/loader-profile.js
 */
import * as noop_js_1 from "../../../../common/util/noop";
import * as observable_property_js_1 from "../../../../common/util/observable-property";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
export class CoreConfig {
  constructor(o) {
    this.logger = hvigor_log_js_1.HvigorLogger.getLogger(CoreConfig.name), this.config = o ? (0, observable_property_js_1.observe)(o, noop_js_1.noop) : (0, observable_property_js_1.observe)({}, noop_js_1.noop);
  }
  getObject(o) {
    return this.config ? this.config[o] : undefined;
  }
  setObject(o, e) {
    Object.keys(this.config[o]).forEach(e => delete this.config[o][e]), Object.keys(e).forEach(r => {
      this.config[o][r] = (0, observable_property_js_1.observe)(e[r], noop_js_1.noop);
    });
  }
}
