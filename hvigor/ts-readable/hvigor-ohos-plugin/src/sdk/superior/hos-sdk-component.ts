/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/superior/hos-sdk-component.js
 */
import * as path from "path";
export class HosSdkComponent {
  constructor(e, t) {
    this._component = e, this._baseDir = path.resolve(e.getLocation(), t);
  }
  getBaseDir() {
    return this._baseDir;
  }
  getComponent() {
    return this._component;
  }
}
