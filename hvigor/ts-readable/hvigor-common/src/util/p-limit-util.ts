/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/p-limit-util.js
 */
export function createPromiseLimit(e) {
  const t = Math.max(Number(e) || 0, 1),
    i = [];
  let r = 0;
  const s = () => {
    if (0 === i.length || r >= t) return;
    const e = i.shift();
    r++, e().finally(() => {
      r--, s();
    });
  };
  return e => new Promise((t, r) => {
    i.push(async () => {
      try {
        const i = await e();
        t(i);
      } catch (e) {
        r(e);
      }
    }), s();
  });
}
