/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/observable-property.js
 */
export function observe(e, t) {
  return "object" != typeof e || null === e ? e : (Object.keys(e).forEach(r => {
    const o = e[r];
    "object" == typeof o && (e[r] = createObservableObject(o, t));
  }), createObservableObject(e, t));
}
function createObservableObject(e, t) {
  return new Proxy(e, {
    set: function (e, r, o) {
      return e[r] = o, t(), true;
    }
  });
}
