/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/resolve/lib/is-core.js
 */
import * as node_pack_list_constant_1 from "./node-pack-list-constant";
const data = node_pack_list_constant_1.NODE_PACK_LIST;
function versionCheck(e) {
  let t = (process.versions && process.versions.node).split("."),
    r = e.split(" "),
    n = r.length > 1 ? r[0] : "=",
    o = (r.length > 1 ? r[1] : r[0]).split(".");
  for (let e = 0; e < 3; ++e) {
    let r = parseInt(t[e] ?? 0, 10),
      s = parseInt(o[e] ?? 0, 10);
    if (r !== s) return "<" === n ? r < s : ">=" === n && r >= s;
  }
  return ">=" === n;
}
function mulVersionCheck(e) {
  const t = e.split(/ ?&& ?/);
  if (0 === t.length) return false;
  for (let e = 0; e < t.length; ++e) if (!versionCheck(t[e])) return false;
  return true;
}
function versionMatchNode(e) {
  if ("boolean" == typeof e) return e;
  if (e && "object" == typeof e) {
    for (let t = 0; t < e.length; ++t) if (mulVersionCheck(e[t])) return true;
    return false;
  }
  return mulVersionCheck(e);
}
function isCorePack(e) {
  return Object.keys(data).filter(t => e === t).length > 0;
}
export function isCore(e) {
  return isCorePack(e) && versionMatchNode(data[e]);
}
