/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/libs-file-cache-util.js
 */
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as so_strip_util_js_1 from "./so-strip-util";
export class LibsFileCacheUtil {
  static getFileHash(t) {
    return (0, hvigor_common_1.hash)(String(t.size + t.mtime.getTime()));
  }
  static async isSourceChanged(t, e, i, s, a) {
    const r = path_1.relative(s, e),
      l = this.isStripped(t, i.debugSymbol, r, s);
    if (this.isStripped(t, a, r, s) !== l) return true;
    const n = i.libs[e];
    return undefined === n || !fs_extra_1.existsSync(e) || n !== LibsFileCacheUtil.getFileHash(fs_extra_1.statSync(e));
  }
  static async isBinxoSourceChanged(t, e, i, s) {
    const a = path_1.relative(s, e);
    if (!this.isBinxoed(t, i.binxoSymbol, a, s)) return true;
    const r = i.libs[e];
    return undefined === r || !fs_extra_1.existsSync(e) || r !== LibsFileCacheUtil.getFileHash(fs_extra_1.statSync(e));
  }
  static isStripped(t, e, i, s) {
    var a;
    return (null === (a = null == e ? undefined : e.strip) || undefined === a || a ? so_strip_util_js_1.SoStripUtil.filesExcluding : so_strip_util_js_1.SoStripUtil.matchedFiles)(t.collectAllLibs, null == e ? undefined : e.exclude, s).some(t => path_1.normalize(t) === path_1.normalize(i));
  }
  static isBinxoed(t, e, i, s) {
    return !!(null == e ? undefined : e.enableAsanBinxo) && so_strip_util_js_1.SoStripUtil.filesExcluding(t.collectAllLibs, null == e ? undefined : e.excludeSoFromBinxo, s).some(t => path_1.normalize(t) === path_1.normalize(i));
  }
  static async isSinkChanged(t, e) {
    const i = e.stripped[t];
    return undefined === i || !fs_extra_1.existsSync(t) || i !== LibsFileCacheUtil.getFileHash(fs_extra_1.statSync(t));
  }
  static async isBinxoLibsChanged(t, e) {
    const i = e.binxo[t];
    return undefined === i || !fs_extra_1.existsSync(t) || i !== LibsFileCacheUtil.getFileHash(fs_extra_1.statSync(t));
  }
  static async generateFileHashesForDirectory(t) {
    if (!t || !path_1.isAbsolute(t)) throw new Error(`Invalid ${t} path. It must be a non-empty absolute path.`);
    const e = {};
    if (!fs_extra_1.existsSync(t)) return e;
    async function i(t, i) {
      try {
        const a = path_1.join(t, i),
          r = fs_extra_1.statSync(a);
        r.isDirectory() ? await s(a) : r.isFile() && (e[a] = LibsFileCacheUtil.getFileHash(r));
      } catch (t) {
        if (!(t instanceof Error && "code" in t && "ENOENT" === t.code)) throw t;
      }
    }
    async function s(t) {
      const e = fs_extra_1.readdirSync(t);
      for (const s of e) await i(t, s);
    }
    return await s(t), e;
  }
  static async refreshLibsFileCache(t, e, i, s) {
    const a = {
      libs: fs_extra_1.existsSync(t) ? await LibsFileCacheUtil.generateFileHashesForDirectory(t) : {},
      stripped: fs_extra_1.existsSync(e) ? await LibsFileCacheUtil.generateFileHashesForDirectory(e) : {},
      debugSymbol: s
    };
    fs_extra_1.ensureDirSync(path_1.dirname(i)), await fs_extra_1.writeFile(i, JSON.stringify(a), "utf8");
  }
  static async refreshBinxoFileCache(t, e, i, s) {
    const a = {
      libs: fs_extra_1.existsSync(t) ? await LibsFileCacheUtil.generateFileHashesForDirectory(t) : {},
      binxo: fs_extra_1.existsSync(e) ? await LibsFileCacheUtil.generateFileHashesForDirectory(e) : {},
      binxoSymbol: s
    };
    fs_extra_1.ensureDirSync(path_1.dirname(i)), await fs_extra_1.writeFile(i, JSON.stringify(a), "utf8");
  }
}
