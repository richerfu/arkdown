/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/add-packagejson-meta-plugin.js
 */
import fs_1 from "fs";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import path_1 from "path";
import * as constants_js_1 from "../util/constants";
import * as utils_1 from "../util/utils";
let packageJsonCache, entryFileCache;
function findPackageJson(e, t) {
  const {
    root: n
  } = path_1.parse(e);
  let o = e;
  for (; o !== n;) {
    const e = (0, utils_1.getOhPackageJsonPath)(path_1.join(o, t));
    if (fs_1.existsSync(e)) {
      if (packageJsonCache[e]) return packageJsonCache[e];
      const t = fs_1.readFileSync(e, "utf-8");
      try {
        return packageJsonCache[e] = {
          pkgJson: (0, hvigor_common_1.parseJsonText)(t),
          pkgPath: o,
          pkgJsonPath: e
        }, packageJsonCache[e];
      } catch (e) {
        return;
      }
    }
    o = path_1.resolve(o, "..");
  }
}
function findEntryFile(e, t) {
  if (entryFileCache[t]) return entryFileCache[t];
  let n;
  return e.exports ? "string" == typeof e.exports ? n = e.exports : "object" == typeof e.exports && (n = function e(t) {
    if (t.import) return t.import;
    if (t["."]) {
      const n = t["."];
      if ("string" == typeof n) return n;
      if ("object" == typeof n) return e(n);
    } else if (t.default) return t.default;
  }(e.exports)) : e.module ? n = e.module : e.main && (n = e.main), entryFileCache[t] = n ?? "index.js", entryFileCache[t];
}
function resolveId(e, t, n) {
  if (!t) return null;
  if (n && n.custom && n.custom["node-resolve"]) {
    const {
      resolved: e
    } = n.custom["node-resolve"];
    if (!e) return null;
    const t = e.id,
      o = this.share.projectConfig?.packageManagerType === constants_js_1.OHPM,
      a = findPackageJson(t, o ? constants_js_1.OH_PACKAGE_JSON5 : constants_js_1.PACKAGE_JSON);
    if (a && a.pkgJson && a.pkgJsonPath && -1 !== t.indexOf(o ? constants_js_1.OH_NODE_MODULES : constants_js_1.NODE_MODULES)) {
      const e = findEntryFile(a.pkgJson, a.pkgJsonPath);
      let n = false;
      if (a.pkgJson.name) {
        const o = path_1.join(a.pkgJson.name, e);
        n = t.endsWith(o);
      }
      return {
        meta: {
          packageJson: a,
          isNodeModuleEntryFile: n
        },
        id: t
      };
    }
  }
  return null;
}
function buildStart() {
  packageJsonCache = {}, entryFileCache = {};
}
export function addPackageJsonMetaPlugin() {
  return {
    name: "addPackageJsonMetaPlugin",
    buildStart: buildStart,
    resolveId: resolveId
  };
}
