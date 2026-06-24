/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/mock-config-utils.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as json_util_js_1 from "./json-util";
export const getMockConfigKey2ModuleInfoMap = (e, t) => {
  const o = {},
    s = json_util_js_1.JsonUtil.getJson5Obj(e);
  if (!s || !Object.keys(s).length) return o;
  const n = new Map();
  return t.getAllDependencies().forEach(e => {
    n.set(e.getDependencyName(), e);
  }), Object.keys(s).forEach(e => {
    const t = e.split(/[/\\]/);
    let s = t.shift();
    if ("." === s[0]) return;
    if ("@" === s[0] && t.length > 0 && (s += `/${t.shift()}`), !n.has(s)) return;
    const r = n.get(s),
      u = r.getDependencyRootPath();
    o[e] = {
      meta: {
        pkgPath: u,
        pkgName: r.getPackageName()
      },
      filePath: t.length ? path_1.resolve(u, ...t) : r.getDependencyMainFilePath()
    };
  }), o;
};
export const getMockConfigSources = (e, t) => {
  const o = [],
    s = json_util_js_1.JsonUtil.getJson5Obj(e);
  return s ? (Object.values(s).forEach(e => {
    const s = e.source;
    if (s) {
      const e = path_1.resolve(t, s);
      fs_extra_1.existsSync(e) && o.push(e);
    }
  }), o) : o;
};
export const getSource2ModuleIdMap = (e, t) => {
  const o = new Map(),
    s = json_util_js_1.JsonUtil.getJson5Obj(e);
  return s ? (Object.values(s).forEach(e => {
    const s = e.source;
    if (s) {
      const e = path_1.resolve(t, s);
      fs_extra_1.existsSync(e) && o.set(s, e);
    }
  }), o) : o;
};
