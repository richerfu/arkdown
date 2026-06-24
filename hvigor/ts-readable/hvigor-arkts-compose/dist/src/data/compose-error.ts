/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/data/compose-error.js
 */
export const ARKTS_COMPILER_ERROR_NAME = "ArkTSCompilerError";
export class ArkTsCompilerError extends Error {
  constructor(r, o) {
    super(r), this.name = ARKTS_COMPILER_ERROR_NAME, o && (this.stack = o);
  }
}
export function throwArkTsCompilerError(...r) {
  throw new ArkTsCompilerError(r.join(" "));
}
export class ExtendedErrorInfo extends Error {
  constructor(r, o) {
    super(r), this.extraInfo = o;
  }
}
