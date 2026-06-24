/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/magic-string-replacement/code-fragment.js
 */
export class CodeFragment {
  constructor(t) {
    this.prefix = "", this.suffix = "", this.isOverwritten = false, this.start = t.start, this.end = t.end, this.sourceCode = t.fragmentSourceCode, this.content = this.sourceCode;
  }
  toString() {
    return this.prefix + this.content + this.suffix;
  }
  isInRange(t) {
    return this.start <= t && t < this.end;
  }
  overwrite(t) {
    this.content = t, this.prefix = "", this.suffix = "", this.isOverwritten = true;
  }
}
