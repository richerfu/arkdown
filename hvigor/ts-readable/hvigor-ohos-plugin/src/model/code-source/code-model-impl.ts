/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/code-source/code-model-impl.js
 */
export class CodeModelImpl {
  constructor(e, t) {
    this._codeType = t, this._srcPath = e;
  }
  getSrcPath() {
    return this._srcPath;
  }
  getCodeType() {
    return this._codeType;
  }
}
