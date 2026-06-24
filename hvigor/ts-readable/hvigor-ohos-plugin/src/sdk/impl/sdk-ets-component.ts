/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-ets-component.js
 */
import path_1 from "path";
import * as sdk_component_js_1 from "../superior/sdk-component";
import * as ets_ark_component_js_1 from "./ets-ark-component";
export class SdkEtsComponent extends sdk_component_js_1.SdkComponent {
  constructor(e) {
    super(e), this._etsLoaderPath = path_1.resolve(this.getBaseDir(), "build-tools", "ets-loader"), this._arkComponent = new ets_ark_component_js_1.EtsArkComponent(this._etsLoaderPath), this._sysCapFilePath = path_1.resolve(this.getBaseDir(), "api", "device-define");
  }
  getEtsLoaderPath() {
    return this._etsLoaderPath;
  }
  getArkVersion() {
    return this._arkComponent.getArkVersion();
  }
  getCompatibleArkVersion(e, t) {
    return this._arkComponent.getArkVersion(e, t);
  }
  getSysCapFilePath() {
    return this._sysCapFilePath;
  }
}
