/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/module-option-path-info.js
 */
import * as build_option_path_info_js_1 from "./build-option-path-info";
import * as build_option_util_js_1 from "../project/build-option/build-option-util";
import * as json_util_js_1 from "../utils/json-util";
class ModuleOptionPathInfo {
  constructor() {
    this.moduleOptPathMap = new Map();
  }
  initModuleOptPathMap(t) {
    const o = json_util_js_1.JsonUtil.getJson5Obj(t);
    this.moduleOptPathMap = (0, build_option_util_js_1.readOpt)(o, t);
  }
  setModuleOptPathMap(t, o, i) {
    0 === this.moduleOptPathMap.size && this.initModuleOptPathMap(t);
    const e = (0, build_option_util_js_1.readOpt)(o, i);
    this.moduleOptPathMap = (0, build_option_path_info_js_1.mergePath)(this.moduleOptPathMap, e);
  }
  getModuleOptPath(t, o) {
    var i, e;
    return 0 === this.moduleOptPathMap.size && this.initModuleOptPathMap(t), null !== (e = null === (i = this.moduleOptPathMap.get(o)) || undefined === i ? undefined : i.srcPath) && undefined !== e ? e : t;
  }
}
export const moduleOptionPathInfo = new ModuleOptionPathInfo();
