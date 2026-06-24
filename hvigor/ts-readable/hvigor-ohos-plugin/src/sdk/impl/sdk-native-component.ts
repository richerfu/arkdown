/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-native-component.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as sdk_component_js_1 from "../superior/sdk-component";
export class SdkNativeComponent extends sdk_component_js_1.SdkComponent {
  constructor(e) {
    super(e);
    const t = (0, hvigor_1.isWindows)() ? "cmake.exe" : "cmake",
      i = (0, hvigor_1.isWindows)() ? "ninja.exe" : "ninja",
      o = (0, hvigor_1.isWindows)() ? "llvm-strip.exe" : "llvm-strip",
      s = (0, hvigor_1.isWindows)() ? "llvm-readelf.exe" : "llvm-readelf";
    this._ninjaTool = path_1.resolve(this.getBaseDir(), "build-tools", "cmake", "bin", i), this._cmakeTool = path_1.resolve(this.getBaseDir(), "build-tools", "cmake", "bin", t), this._nativeToolchain = path_1.resolve(this.getBaseDir(), "build", "cmake", "ohos.toolchain.cmake"), this._llvmStrip = path_1.resolve(this.getBaseDir(), "llvm", "bin", o), this._llvmReadElf = path_1.resolve(this.getBaseDir(), "llvm", "bin", s);
  }
  getCmakeTool() {
    return this._cmakeTool;
  }
  getNinjaTool() {
    return this._ninjaTool;
  }
  getNativeToolchain() {
    return this._nativeToolchain;
  }
  getLlvmStrip() {
    return this._llvmStrip;
  }
  getLlvmReadElf() {
    return this._llvmReadElf;
  }
}
