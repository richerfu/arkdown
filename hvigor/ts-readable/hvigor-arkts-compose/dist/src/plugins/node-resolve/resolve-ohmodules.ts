/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/resolve-ohmodules.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as path_cache_stat_js_1 from "../../util/path-cache-stat";
const OH_MODULES = "oh_modules",
  DOT_OHPM = ".ohpm",
  symlinkMap = {};
function updateSymlinkMap(t) {
  const e = fs_1.realpathSync(t);
  if (e === t || !e) return;
  const o = symlinkMap[e] || [];
  o.push(t), symlinkMap[e] = o;
}
export function resolveAllOhModules(t, e) {
  resolveProjectOhModules(t, e);
  const o = path_1.join(t, "build-profile.json5");
  if (path_cache_stat_js_1.PathCacheStat.exists(o, e)) {
    const s = fs_1.readFileSync(o, "utf-8"),
      r = (0, hvigor_common_1.parseJsonText)(s).modules;
    for (const o of r) {
      resolveProjectOhModules(path_1.join(t, o.srcPath), e);
    }
  } else {
    const o = fs_1.readdirSync(t);
    for (const s of o) {
      resolveProjectOhModules(path_1.join(t, s), e);
    }
  }
  return symlinkMap;
}
function resolveProjectOhModules(t, e) {
  const o = path_1.join(t, OH_MODULES);
  if (!isExistDir(o, e)) return;
  resolveOhmodules(o, e);
  const s = path_1.join(o, ".ohpm");
  if (!isExistDir(s, e)) return;
  const r = fs_1.readdirSync(s);
  for (const t of r) {
    const o = path_1.join(s, t, OH_MODULES);
    isExistDir(o, e) && resolveOhmodules(o, e);
  }
}
function isExistDir(t, e) {
  try {
    if (path_cache_stat_js_1.PathCacheStat.isDirectory(t, e)) return true;
  } catch (t) {
    return false;
  }
  return false;
}
function resolveOhmodules(t, e) {
  const o = fs_1.readdirSync(t);
  for (const s of o) {
    if (".ohpm" === s) continue;
    const o = path_1.join(t, s);
    if (path_cache_stat_js_1.PathCacheStat.isDirectory(o, e)) if (s.startsWith("@")) {
      const t = fs_1.readdirSync(o);
      for (const s of t) {
        const t = path_1.join(o, s);
        path_cache_stat_js_1.PathCacheStat.isDirectory(t, e) && updateSymlinkMap(t);
      }
    } else updateSymlinkMap(o);
  }
}
