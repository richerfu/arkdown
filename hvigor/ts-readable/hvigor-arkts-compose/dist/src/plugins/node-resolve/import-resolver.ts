/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/import-resolver.js
 */
import path_1 from "path";
import * as constants_js_1 from "../../util/constants";
import * as path_cache_stat_js_1 from "../../util/path-cache-stat";
import * as util_js_1 from "./util";
import * as index_js_1 from "./index";
export class ImportResolver {
  constructor(e, t, s) {
    this.resolve = e => {
      const t = (0, util_js_1.getPackageName)(e),
        s = t ? t.split("\\")[0] : null;
      if (!s) return;
      let a, i;
      for (const e of this.depName2DepInfos.keys()) e.toLowerCase() === s.toLowerCase() && (a = e, i = this.depName2DepInfos.get(e));
      if (!i || !a || "hsp" !== i.dependencyType && !i.isByteCodeHar) return;
      const r = s === a,
        n = path_1.normalize(e);
      let h = "";
      const o = i.pkgRootPath;
      let p = this.getReferenceNamePath(o, e, s);
      n.endsWith(path_1.sep) && (p = `${p}${path_1.sep}`);
      const l = `^${a}[\\\\/]*$`;
      return h = this.getResReferencePath(r, l, n, p, e) ?? "", h ? {
        packageName: i.pkgName,
        normalizedPath: this.removeFileExtensionFromPath(path_1.join(i.pkgName, path_1.relative(o, h)))
      } : undefined;
    }, this.depName2DepInfos = e, this.caseSensitiveCheck = t, this.getInfo = s;
  }
  removeFileExtensionFromPath(e) {
    const t = path_1.dirname(e),
      s = path_1.basename(e);
    let a = "";
    a = s.endsWith(".d.ets") ? ".d.ets" : s.endsWith(".d.ts") ? ".d.ts" : path_1.extname(e);
    const i = s.slice(0, -a.length);
    return path_1.join(t, i);
  }
  pathIsDirectory(e) {
    const t = path_cache_stat_js_1.PathCacheStat.getStat(e, this.getInfo);
    return !!t.exists && t.isDirectory;
  }
  pathIsFile(e) {
    const t = constants_js_1.SUFFIX_EXTENSIONS.map(t => e + t);
    t.unshift(e);
    for (let e = 0; e < t.length; e++) if (this.pathIsFilePath(t[e])) return true;
    return false;
  }
  pathIsFilePath(e) {
    const t = path_cache_stat_js_1.PathCacheStat.getStat(e, this.getInfo);
    return !!t.exists && t.isFile;
  }
  getReferenceNamePath(e, t, s) {
    const a = new RegExp(`^${s}[/\\\\]*`),
      i = path_1.normalize(t.replace(a, "")).split(path_1.sep);
    return path_1.resolve(e, i.join(path_1.sep));
  }
  getResReferencePath(e, t, s, a, i) {
    let r = "";
    if (e) {
      if (new RegExp(t).test(s)) return;
      if (this.pathIsFile(a)) return;
      this.pathIsDirectory(a) && (r = index_js_1.importee2filePath.get(i) ?? "");
    }
    if (!e && !this.caseSensitiveCheck) if (new RegExp(t, "i").test(s)) r = index_js_1.importee2filePath.get(i) ?? "";else {
      if (this.pathIsFile(a)) return;
      this.pathIsDirectory(a) && (r = index_js_1.importee2filePath.get(i) ?? "");
    }
    return r;
  }
}
