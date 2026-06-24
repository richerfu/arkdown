/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor-error.js
 */
export class AdaptorError extends Error {
  constructor(r, o) {
    super(r), this.name = AdaptorError.NAME, o && (this.stack = o);
  }
}
AdaptorError.NAME = "AdaptorError";
