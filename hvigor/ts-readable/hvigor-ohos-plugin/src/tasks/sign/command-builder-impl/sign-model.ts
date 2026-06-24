/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/sign-model.js
 */
export class SignModel {
  constructor(t, e, i) {
    this._signTypeEnum = t, this._inputFilePath = e, this._outputFilePath = i;
  }
  getSignType() {
    return this._signTypeEnum;
  }
  getInputFilePath() {
    return this._inputFilePath;
  }
  getOutPutFilePath() {
    return this._outputFilePath;
  }
}
