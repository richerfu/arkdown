/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-manager/extended-error-info.js
 */
export class ExtendedErrorInfo extends Error {
  constructor(r, e, o) {
    super(r), this.extraInfo = e, o && (this.stack = o);
  }
}
