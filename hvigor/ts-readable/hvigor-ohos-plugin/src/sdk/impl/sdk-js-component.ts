/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-js-component.js
 */
import path_1 from "path";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as sdk_component_js_1 from "../superior/sdk-component";
import * as js_ark_component_js_1 from "./js-ark-component";
export class SdkJsComponent extends sdk_component_js_1.SdkComponent {
  constructor(e) {
    super(e), this._jsLoaderPath = path_1.resolve(this.getBaseDir(), "build-tools", "ace-loader"), this._hapToBinPath = path_1.resolve(this.getBaseDir(), "build-tools", "binary-tools", sdk_const_js_1.ToolChainsConst.HAP_TO_BIN_TOOL_JAR), this._sysCapFilePath = path_1.resolve(this.getBaseDir(), "api", "device-define"), this.arkComponent = new js_ark_component_js_1.JsArkComponent(this._jsLoaderPath);
  }
  getJsLoaderPath() {
    return this._jsLoaderPath;
  }
  getArkVersion() {
    return this.arkComponent.getArkVersion();
  }
  getCompatibleArkVersion(e, t) {
    return this.arkComponent.getArkVersion(e, t);
  }
  getSysCapFilePath() {
    return this._sysCapFilePath;
  }
  getHapToBinFilePath() {
    return this._hapToBinPath;
  }
}
