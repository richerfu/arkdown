/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/file-util.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
export function traverseFilesSync(e, t = null) {
  const r = [];
  return function e(s) {
    const n = fs_extra_1.readdirSync(s);
    for (const o of n) {
      const n = path_1.join(s, o),
        u = fs_extra_1.statSync(n);
      u.isDirectory() ? e(n) : u.isFile() && shouldIncludeFile(n, t) && r.push(n);
    }
  }(e), r;
}
export function shouldIncludeFile(e, t) {
  if (!t) return true;
  const r = path_1.extname(e);
  return "string" == typeof t ? r === t : Array.isArray(t) ? t.some(e => r === e) : t instanceof RegExp ? t.test(e) : "function" != typeof t || t(e, r);
}
