/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/hash-util.js
 */
import crypto_1 from "crypto";
import fs_1 from "fs";
export const createHash = (e = "sha256") => crypto_1.createHash(e);
export const hash = (e, t) => (0, createHash)(t).update(e).digest("hex");
export const hashFile = (e, t) => {
  if (fs_1.existsSync(e)) return (0, hash)(fs_1.readFileSync(e, "utf-8"), t);
};
export const getSafeJsonStringIfy = e => {
  const t = new WeakSet();
  function s(e) {
    if (null === e || "object" != typeof e) return "bigint" == typeof e ? null : e;
    if (t.has(e)) return null;
    if (t.add(e), Array.isArray(e)) return e.map(s);
    const r = {};
    return Object.keys(e).sort().forEach(t => {
      "function" != typeof e[t] && "symbol" != typeof e[t] && (r[t] = s(e[t]));
    }), r;
  }
  return JSON.stringify(e, function (e, t) {
    return s(t);
  });
};
export const hashObject = e => (0, createHash)().update((0, getSafeJsonStringIfy)(e)).digest("hex");
