/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/path-util.js
 */
import path_1 from "path";
export function isSubPath(t, e) {
  try {
    const r = path_1.relative(t, e);
    if ("" === r) return true;
    const i = r.split(path_1.sep);
    for (const t of i) if (".." !== t) return false;
    return true;
  } catch (t) {
    return false;
  }
}
export function isChildPath(t, e) {
  if (t.length < e.length) return false;
  if (!t.startsWith(e)) return false;
  if (t.length === e.length) return true;
  if (e.endsWith(path_1.sep)) return true;
  const r = t.charCodeAt(e.length);
  return 47 === r || 92 === r;
}
