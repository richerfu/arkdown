/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/resolve/lib/caller.js
 */
export function caller() {
  const r = Error.prepareStackTrace;
  Error.prepareStackTrace = function (r, e) {
    return e;
  };
  const e = new Error().stack;
  return Error.prepareStackTrace = r, e[2].getFileName();
}
