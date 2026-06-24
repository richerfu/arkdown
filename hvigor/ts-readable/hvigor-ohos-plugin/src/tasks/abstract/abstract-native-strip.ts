/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-native-strip.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as binxo_validation_service_factory_js_1 from "../../utils/validate/binxo/binxo-validation-service-factory";
import * as abstract_build_native_js_1 from "../abstract-build-native";
var __decorate = this && this.__decorate || function (t, i, e, o) {
  var r,
    a = arguments.length,
    s = a < 3 ? i : null === o ? o = Object.getOwnPropertyDescriptor(i, e) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) s = Reflect.decorate(t, i, e, o);else for (var n = t.length - 1; n >= 0; n--) (r = t[n]) && (s = (a < 3 ? r(s) : a > 3 ? r(i, e, s) : r(i, e)) || s);
  return a > 3 && s && Object.defineProperty(i, e, s), s;
};
export class AbstractNativeStrip extends abstract_build_native_js_1.AbstractBuildNative {
  constructor(t, i) {
    var e, o, r, a;
    super(t, i), this.nativeLibOption = null === (e = t.getBuildOption()) || undefined === e ? undefined : e.nativeLib, this.excludeSoFromBinxo = null === (o = this.nativeLibOption) || undefined === o ? undefined : o.excludeSoFromBinXO, this.debugSymbol = null === (r = this.nativeLibOption) || undefined === r ? undefined : r.debugSymbol, this.exclude = null === (a = this.debugSymbol) || undefined === a ? undefined : a.exclude, this.intermediatesProcessLibs = this.pathInfo.getIntermediatesProcessLibs(), this.doNativeBinxoLibs = this.pathInfo.getIntermediatesDoNativeBinxo(), this.strippedNativeLibs = this.pathInfo.getIntermediatesStrippedLibsDir(), this.moduleBuildPath = this.pathInfo.getModuleBuildPath(), this.cacheFilePath = path_1.resolve(this.pathInfo.getIntermediatesPatch(), build_directory_const_js_1.BuildArtifactConst.BASE_NATIVE_LIBS_JSON), this.cacheBinxoFilePath = path_1.resolve(this.pathInfo.getIntermediatesPatch(), build_directory_const_js_1.BuildArtifactConst.BINXO_NATIVE_LIBS_JSON), this.binxoValidationService = binxo_validation_service_factory_js_1.BinxoValidationServiceFactory.createBinxoValidationService(t);
  }
  isBinxoValid() {
    return this.binxoValidationService.isBinxoValid();
  }
}
__decorate([(0, hvigor_1.Input)()], AbstractNativeStrip.prototype, "excludeSoFromBinxo", undefined), __decorate([(0, hvigor_1.Input)()], AbstractNativeStrip.prototype, "debugSymbol", undefined), __decorate([(0, hvigor_1.InputFile)({
  isDirectory: true
})], AbstractNativeStrip.prototype, "intermediatesProcessLibs", undefined);
