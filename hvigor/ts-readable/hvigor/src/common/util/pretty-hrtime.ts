/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/pretty-hrtime.js
 */
const shortDesc = ["h", "min", "s", "ms", "μs", "ns"],
  longDesc = ["hour", "minute", "second", "millisecond", "microsecond", "nanosecond"],
  convertArray = [3600, 60, 1, 1e6, 1e3, 1];
function default_1(e, r) {
  if (!Array.isArray(e) || 2 !== e.length) return "";
  if ("number" != typeof e[0] || "number" != typeof e[1]) return "";
  const t = (null == r ? undefined : r.verbose) || false,
    o = (null == r ? undefined : r.precise) || false;
  if (e[1] < 0) {
    const r = e[0] + e[1] / 1e9;
    e[0] = parseInt(r.toString()), e[1] = 1e9 * parseFloat((r % 1).toPrecision(9));
  }
  return doCalc(e, t, o);
}
function doCalc(e, r, t) {
  let o = "";
  for (let n = 0; n < 6; n++) {
    let s = e[n < 3 ? 0 : 1];
    3 !== n && 0 !== n && (s %= convertArray[n - 1]), 2 === n && (s += e[1] / 1e9);
    let c,
      i = s / convertArray[n];
    if (!(i < 1)) {
      if (r && (i = Math.floor(i)), t) c = i.toString();else {
        const e = i >= 10 ? 0 : 2;
        c = i.toFixed(e);
      }
      if (c.indexOf(".") > -1 && "0" === c[c.length - 1] && (c = c.replace(/\.?0+$/, "")), o && (o += " "), o += c, r ? (o += ` ${longDesc[n]}`, "1" !== c && (o += "s")) : o += ` ${shortDesc[n]}`, !r) break;
    }
  }
  return o;
}
export default default_1;
