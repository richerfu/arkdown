/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-java-component.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as hos_sdk_component_js_1 from "../superior/hos-sdk-component";
export class SdkJavaComponent extends hos_sdk_component_js_1.HosSdkComponent {
  constructor(e) {
    super(e, "java");
  }
  getRestoolPath() {
    const e = (0, hvigor_1.isWindows)() ? "restool.exe" : "restool";
    return path_1.resolve(this.getBaseDir(), SdkJavaComponent.BUILD_TOOLS, e);
  }
  getReleaseType() {
    return this.getComponent().getReleaseType();
  }
}
SdkJavaComponent.BUILD_TOOLS = "build-tools";
