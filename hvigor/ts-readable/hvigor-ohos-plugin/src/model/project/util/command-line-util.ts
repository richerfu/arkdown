/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/util/command-line-util.js
 */
export function parseTargets(e) {
  const s = new Map();
  if (undefined === e) return s;
  const t = e.split(",");
  for (const e of t) {
    const t = e.split("@");
    let r = 0;
    const o = ["all"];
    for (const e of t.slice(1)) "" !== e && (o[r++] = e);
    s.set(t[0], o);
  }
  return s;
}
