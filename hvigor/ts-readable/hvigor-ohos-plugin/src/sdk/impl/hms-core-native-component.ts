/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/hms-core-native-component.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as sdk_component_js_1 from "../superior/sdk-component";
export class HmsCoreNativeComponent extends sdk_component_js_1.SdkComponent {
  constructor(e) {
    super(e);
    const o = (0, hvigor_1.isWindows)() ? "binxo.exe" : "binxo";
    this._binxoTool = path_1.resolve(this.getBaseDir(), "BinXO", o);
  }
  getHmsToolchainFile() {
    return path_1.resolve(this.getBaseDir(), "build", "cmake", "hmos.toolchain.cmake");
  }
  getHmsBiShengToolchainFile() {
    return path_1.resolve(this.getBaseDir(), "build", "cmake", "hmos.toolchain.bisheng.cmake");
  }
  getSdkBinxoTool() {
    return this._binxoTool;
  }
}
