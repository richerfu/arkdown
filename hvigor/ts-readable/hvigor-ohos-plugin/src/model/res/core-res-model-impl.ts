/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/res/core-res-model-impl.js
 */
import fs_1 from "fs";
export class CoreResModelImpl {
  constructor(e, t) {
    this.jsonPath = e, this.resourcePath = t;
  }
  getJsonContent() {
    return fs_1.readFileSync(this.jsonPath, "utf-8");
  }
  getResourcePath() {
    return this.resourcePath;
  }
  getJsonPath() {
    return this.jsonPath;
  }
}
