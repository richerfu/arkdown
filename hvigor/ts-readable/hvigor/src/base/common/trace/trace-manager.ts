/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/trace-manager.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import fs_extra_1 from "fs-extra";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as path_util_js_1 from "../../../common/util/path-util";
import * as replacer_js_1 from "../../../common/util/replacer";
export class TraceManager {
  constructor() {
    this.callBackList = {}, this.data = {}, this.callBackList = {};
  }
  trace(e, t, a) {
    this.data[e] = t, "function" == typeof a && (this.callBackList[e] = a);
  }
  flush() {
    const e = (0, wdk_1.cloneDeep)(this.data);
    for (const e in this.callBackList) this.callBackList[e]();
    const t = path_1.resolve(path_util_js_1.PathUtil.getHvigorCacheDir(), "./outputs/logs/details"),
      a = path_1.resolve(t, "details.json");
    fs_extra_1.ensureDirSync(t);
    try {
      fs_extra_1.writeFileSync(a, JSON.stringify(e, replacer_js_1.replacer, 2));
    } catch (e) {}
  }
  static transformKey(e) {
    return e.replace(/\./g, "_");
  }
  static anonymize(e) {
    return (0, hvigor_common_1.hash)(e);
  }
  static trace(e, t, a) {
    traceManager.trace(e, t, a);
  }
}
export const traceManager = new TraceManager();
