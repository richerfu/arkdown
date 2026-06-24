/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/base-command-builder.js
 */
export class BaseCommandBuilder {
  constructor(s) {
    this.commandList = s;
  }
  build() {
    return this.commandList;
  }
  addAllParams(s) {
    return this.commandList = [...this.commandList, ...s], this.commandList;
  }
}
