/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-arkuix-component.js
 */
import path_1 from "path";
import * as sdk_component_js_1 from "../superior/sdk-component";
export class SdkArkUIXComponent extends sdk_component_js_1.SdkComponent {
  constructor(t) {
    super(t), this.ARKUI_X_ROOT = "arkui-x";
  }
  getArkUIXRootPath() {
    return path_1.resolve(this.getBaseDir(), this.ARKUI_X_ROOT);
  }
}
