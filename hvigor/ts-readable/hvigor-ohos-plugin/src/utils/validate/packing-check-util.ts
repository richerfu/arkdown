/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/packing-check-util.js
 */
import * as glob_1 from "glob";
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as file_util_js_1 from "../file-util";
export class PackingCheckUtil {
  static getResourcePaths(e) {
    var t;
    const r = e.getTargetData().getPathInfo().getModulePath(),
      s = null === (t = e.getTargetData().getTargetOpt().resource) || undefined === t ? undefined : t.directories;
    return (null == s ? undefined : s.length) ? s.map(e => file_util_js_1.FileUtil.convertToAbsolutePath(e, r)) : [e.getTargetData().getModuleSourceSetModel().getTargetResPath()];
  }
  static collectSourceCodeFiles(e, t) {
    const r = [".c", ".h", ".cpp", ".cs", ".java", ".rs", ".py", ".go", ".ets", ".js", ".ts"];
    if (!fs_extra_1.existsSync(e)) return [];
    return glob_1.glob.sync("**/*", {
      cwd: e,
      absolute: false,
      dot: true
    }).filter(null != t ? t : e => r.some(t => e.endsWith(t)));
  }
  static recoverOriginalPath(e, t) {
    return e.filter(e => fs_extra_1.existsSync(path_1.resolve(t, e))).map(e => path_1.resolve(t, e));
  }
  static recoverPackagedFiles(e, t) {
    if (!fs_extra_1.existsSync(t)) return [];
    let r;
    return this.resourceFilesCache.has(t) ? r = this.resourceFilesCache.get(t) : (r = new Set(glob_1.glob.sync("**/*", {
      cwd: t,
      absolute: false,
      dot: true
    })), this.resourceFilesCache.set(t, r)), e.filter(e => r.has(e));
  }
  static clearResourceFilesCache() {
    this.resourceFilesCache.clear();
  }
}
PackingCheckUtil.resourceFilesCache = new Map();
