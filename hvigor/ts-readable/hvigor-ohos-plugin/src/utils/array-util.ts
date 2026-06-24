/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/array-util.js
 */
export function getIntersection(e, t) {
  return e.filter(function (e) {
    return t.indexOf(e) > -1;
  });
}
export function getElementFromArr(e, t, r = "name") {
  if (undefined !== e) for (const n of e) if (n[r] === t) return n;
}
export function findDuplicateElement(e) {
  const t = new Set(),
    r = new Set();
  return e.forEach(e => {
    r.has(e) ? t.add(e) : r.add(e);
  }), [...t];
}
export function checkArrayIsSame(e, t) {
  if (e.length === t.length) {
    for (let r = 0; r < e.length; r++) if (e[r] !== t[r]) return false;
    return true;
  }
  return false;
}
export function checkArrayElementIsSame(e, t) {
  return e.length === t.length && JSON.stringify(e.sort()) === JSON.stringify(t.sort());
}
export function isSubset(e, t) {
  return t.every(t => e.includes(t));
}
export function complementarySet(e, t) {
  return e.filter(e => !t.includes(e));
}
export function checkIntersection(e, t) {
  return !(!e || !t) && e.filter(e => t.includes(e)).length > 0;
}
export function getUnionSet(e = [], t = []) {
  return Array.from(new Set([...e, ...t]));
}
export function getIntersectedSet(e, t) {
  if (!e || !t) return new Set();
  const r = Array.from(e).filter(e => t.has(e));
  return new Set(r);
}
