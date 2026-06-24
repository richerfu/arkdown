/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/build-option-path-info.js
 */
import os_1 from "os";
import * as wdk_1 from "@baize/wdk";
import * as build_option_util_js_1 from "../project/build-option/build-option-util";
import * as array_util_js_1 from "../utils/array-util";
import * as find_target_product_js_1 from "./find-target-product";
import * as project_ohos_config_manager_js_1 from "./global/project-ohos-config-manager";
import * as json_util_js_1 from "../utils/json-util";
class BuildOptionPath {
  constructor() {
    this.productOptPath = new Map(), this.buildOptionPath = {}, this.signConfigPath = new Map(), this.appConfigPath = new Map();
  }
  setAppConfigPath(t, i, e) {
    0 === this.appConfigPath.size && this.initAppConfigPath(t);
    const o = (0, build_option_util_js_1.readOpt)(null == i ? undefined : i.app, e);
    this.appConfigPath = mergePath(this.appConfigPath, o);
  }
  initAppConfigPath(t) {
    const i = json_util_js_1.JsonUtil.getJson5Obj(t);
    this.appConfigPath = (0, build_option_util_js_1.readOpt)(null == i ? undefined : i.app, t);
  }
  getAppConfigPath(t, i) {
    var e, o;
    return 0 === this.appConfigPath.size && this.initAppConfigPath(t), null !== (o = null === (e = this.appConfigPath.get(i)) || undefined === e ? undefined : e.srcPath) && undefined !== o ? o : "app.json5 file of the project";
  }
  setProductOptPath(t, i) {
    const e = (0, wdk_1.cloneDeep)(this.productOptPath.get(i));
    e && 0 !== e.size ? this.productOptPath.set(i, mergePath(e, t)) : this.productOptPath.set(i, t);
  }
  setBuildOptionPath(t, i, e) {
    this.buildOptionPath[t] || (this.buildOptionPath[t] = new Map());
    const o = (0, wdk_1.cloneDeep)(this.buildOptionPath[t].get(i));
    o && 0 !== o.size ? this.buildOptionPath[t].set(i, mergePath(o, e, t)) : this.buildOptionPath[t].set(i, e);
  }
  getProductOptPath(t) {
    return this.productOptPath.get(t);
  }
  getSignConfigPath(t, i) {
    var e, o;
    const n = [t.getProject().getBuildProfilePath(), " If there are no issues in this file,  perhaps you have used 'overrides' or 'hook' in 'project hvigorfile.ts', please check it."].join(`${os_1.EOL}\t`);
    return null !== (o = null === (e = this.signConfigPath.get(i)) || undefined === e ? undefined : e.srcPath) && undefined !== o ? o : n;
  }
  initSignConfig(t, i) {
    var e;
    let o = new Map();
    const n = (0, build_option_util_js_1.readOpt)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.signingConfig, t.getProject().getBuildFilePath() + ".ts"),
      s = (0, find_target_product_js_1.findTargetProduct)(t),
      r = t.getProfileOpt().app,
      a = s.signingConfig;
    if (!a) return void (this.signConfigPath = mergePath(this.signConfigPath, n));
    const h = (0, array_util_js_1.getElementFromArr)(null == r ? undefined : r.signingConfigs, a.substring(a.lastIndexOf(".") + 1));
    if (!h) return void (this.signConfigPath = mergePath(this.signConfigPath, n));
    const u = (0, build_option_util_js_1.readOpt)(h, i);
    o = (0, build_option_util_js_1.mergeBuildOptPath)(u, n), this.signConfigPath = mergePath(this.signConfigPath, o);
  }
  getTargetBuildOptPath(t, i, e) {
    var o, n, s, r;
    return null !== (r = null === (s = null === (n = null === (o = this.buildOptionPath[t.getName()]) || undefined === o ? undefined : o.get(i)) || undefined === n ? undefined : n.get(e)) || undefined === s ? undefined : s.srcPath) && undefined !== r ? r : [t.getProfilePath(), " If there are no issues in this file, perhaps you have used 'overrides' in 'hvigorfile.ts', please check it."].join(`${os_1.EOL}\t`);
  }
}
export function mergePath(t, i, e) {
  for (const [o, n] of t) i.has(o) && (t = mergeKeyToMap(t, i, o, e));
  return addKeyToMap(t, i);
}
function mergeKeyToMap(t, i, e, o) {
  const n = t.get(e),
    s = i.get(e);
  if (n && s && s.value === n.value && o) {
    const i = n.srcPath,
      r = s.srcPath;
    !i.includes(o) && r.includes(o) && t.set(e, s);
  } else n && s && s.value !== n.value && t.set(e, s);
  return t;
}
function addKeyToMap(t, i) {
  for (const [e, o] of i) t.has(e) || t.set(e, o);
  return t;
}
export function getHookFilePath() {
  const t = new Error().stack,
    i = "In the hvigorfile.ts or hvigorconfig.ts file of the module or project or in the plug-in";
  if (t) {
    const e = /\(([^()]*hvigorfile\.ts[^()]*)\)/,
      o = t.match(e);
    return o ? o[1] : i;
  }
  return i;
}
export const buildOptionPath = new BuildOptionPath();
