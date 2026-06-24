/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/path-util.js
 */
import path_1 from "path";
import process_1 from "process";
import fs_extra_1 from "fs-extra";
import * as extra_config_js_1 from "../../base/data/extra-config";
import * as const_js_1 from "../const/const";
import * as path_const_js_1 from "../const/path-const";
import * as hvigor_config_loader_js_1 from "./hvigor-config-loader";
export class PathUtil {
  static getHvigorCacheDir(t) {
    var e;
    let a = undefined !== process_1.env.config ? JSON.parse(process_1.env.config)[const_js_1.BUILD_CACHE_DIR] : null !== (e = (0, extra_config_js_1.getExtraConfig)(const_js_1.BUILD_CACHE_DIR)) && undefined !== e ? e : PathUtil.getCommandHvigorCacheDir();
    const r = path_const_js_1.HVIGOR_PROJECT_CACHE_PATH;
    if (!a && (a = hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(const_js_1.HVIGOR_CACHE_DIR_KEY), !a)) return r;
    if (!path_1.isAbsolute(a)) return t && !this.hvigorCacheDirHasLogged && (t.warn(`Invalid custom hvigor data dir:${a}`), this.hvigorCacheDirHasLogged = true), r;
    if (fs_extra_1.existsSync(a)) {
      const e = fs_extra_1.realpathSync.native(a);
      t && path_1.normalize(a) !== path_1.normalize(e) && !this.hvigorCacheDirHasLogged && (t.warn(`The custom .hvigor cache path '${a}' has a case mismatch with the actual directory. It is recommended to use the exact same letter case as the existing path.`), this.hvigorCacheDirHasLogged = true), a = path_1.normalize(e);
    }
    return path_1.resolve(a, path_1.basename(process_1.cwd()), const_js_1.HVIGOR_PROJECT_DIR_NAME);
  }
  static checkCopyPathIsSame(t, e) {
    const a = PathUtil.getStatsSync(t),
      r = PathUtil.getStatsSync(e);
    return !(!r || !a) && !!PathUtil.areIdentical(a, r);
  }
  static getStatsSync(t) {
    let e;
    try {
      e = fs_extra_1.statSync(t);
    } catch (t) {
      return null;
    }
    return e;
  }
  static areIdentical(t, e) {
    return e.ino && e.dev && e.ino === t.ino && e.dev === t.dev;
  }
  static getCommandHvigorCacheDir() {
    return process_1.argv.forEach(t => {
      t.startsWith(const_js_1.BUILD_CACHE_DIR) && (process_1.env.BUILD_CACHE_DIR = t.substring(t.indexOf("=") + 1));
    }), process_1.env.BUILD_CACHE_DIR;
  }
  static getReportDirPath() {
    return path_1.resolve(PathUtil.getHvigorCacheDir(), "report");
  }
}
PathUtil.hvigorCacheDirHasLogged = false;
