/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/time-util.js
 */
import pretty_hrtime_js_1 from "../../common/util/pretty-hrtime";
const timeArray = ["h", "min", "s", "ms", "μs", "ns"],
  unit2Index = new Map([["hour", 0], ["hours", 0], ["minute", 1], ["minutes", 1], ["second", 2], ["seconds", 2], ["millisecond", 3], ["milliseconds", 3], ["microsecond", 4], ["microseconds", 4], ["nanosecond", 5], ["nanoseconds", 5]]);
export function splitTime(e) {
  const t = e.split(" ");
  if (t.length < 1 || t.length % 2) return e;
  let r = "";
  for (let e = 0; e < t.length; e += 2) {
    let i = t[e];
    const o = unit2Index.get(t[e + 1]),
      n = timeArray[o];
    if (0 === e && o > 3) return "1 ms ";
    if (o > 3) break;
    "ms" === n && e < t.length - 2 && (i = String(parseInt(i) + 1)), r += `${i} ${n} `;
  }
  return r;
}
export function formatTimeToNumber(e) {
  return e ? 1e9 * e[0] + e[1] : Number(process.hrtime.bigint());
}
export function formatTimeToNumPair(e) {
  const t = Math.floor(e / 1e9);
  return [t, e - 1e9 * t];
}
export function getIntersect(e) {
  if (0 === e.length) return 0;
  e.sort((e, t) => e[0] - t[0]);
  let t = e[0],
    r = 0;
  for (let i = 1; i < e.length; i++) if (t[1] >= e[i][0]) t[1] = Math.max(t[1], e[i][1]);else {
    r += t[1] - t[0], t = e[i];
  }
  return r += t[1] - t[0], r;
}
export function formatTime(e) {
  return splitTime((0, pretty_hrtime_js_1)(e, {
    verbose: true
  }));
}
export function isHvigorLogWithTime(e) {
  if (e.length < 35) return false;
  return null !== e.substring(0, 35).match(/\[[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9]\.[0-9][0-9][0-9]] > hvigor /);
}
