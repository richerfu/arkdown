/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/path-cache-stat.js
 */
import fs_1 from "fs";
export class PathCacheStat {
  static isDirectory(t, e) {
    return e?.(t).isDirectory ?? fs_1.statSync(t).isDirectory();
  }
  static isFile(t, e) {
    return e?.(t).isFile ?? fs_1.statSync(t).isFile();
  }
  static exists(t, e) {
    return e?.(t).exists ?? fs_1.existsSync(t);
  }
  static getStat(t, e) {
    if (e) return e(t);
    try {
      const e = fs_1.statSync(t);
      return {
        exists: true,
        isDirectory: e.isDirectory(),
        isFile: e.isFile(),
        mtimeMs: e.mtimeMs,
        size: e.size
      };
    } catch (t) {
      if ("ENOENT" === t.code) return {
        exists: false,
        isDirectory: false,
        isFile: false,
        mtimeMs: -1,
        size: -1
      };
      throw t;
    }
  }
}
