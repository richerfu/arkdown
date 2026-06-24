/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/global/core-product.js
 */
import * as hvigor_1 from "@ohos/hvigor";
var __decorate = this && this.__decorate || function (t, e, r, u) {
  var o,
    d = arguments.length,
    a = d < 3 ? e : null === u ? u = Object.getOwnPropertyDescriptor(e, r) : u;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(t, e, r, u);else for (var n = t.length - 1; n >= 0; n--) (o = t[n]) && (a = (d < 3 ? o(a) : d > 3 ? o(e, r, a) : o(e, r)) || a);
  return d > 3 && a && Object.defineProperty(e, r, a), a;
};
export class CoreProduct {
  constructor(t, e) {
    var r;
    this.productBuildOpt = t, this.projectModel = e, this.outputArtifactName = null === (r = t.output) || undefined === r ? undefined : r.artifactName, this.transformToCoreProduct();
  }
  transformToCoreProduct() {
    var t, e;
    this.productName = this.productBuildOpt.name, this.bundleName = null !== (t = this.productBuildOpt.bundleName) && undefined !== t ? t : this.projectModel.getDefaultBundleName(), this.bundleType = null !== (e = this.productBuildOpt.bundleType) && undefined !== e ? e : this.projectModel.getBundleType();
  }
  getBundleName() {
    return this._getBundleName();
  }
  getBundleNameInternal() {
    return this._getBundleName();
  }
  _getBundleName() {
    return this.bundleName;
  }
  getBundleType() {
    return this._getBundleType();
  }
  getBundleTypeInternal() {
    return this._getBundleType();
  }
  _getBundleType() {
    return this.bundleType;
  }
  getProductName() {
    return this._getProductName();
  }
  getProductNameInternal() {
    return this._getProductName();
  }
  _getProductName() {
    return this.productName;
  }
  getOutputArtifactName() {
    return this._getOutputArtifactName();
  }
  getOutputArtifactNameInternal() {
    return this._getOutputArtifactName();
  }
  _getOutputArtifactName() {
    return this.outputArtifactName;
  }
}
__decorate([hvigor_1.TrackAPI], CoreProduct.prototype, "getBundleName", null), __decorate([hvigor_1.TrackAPI], CoreProduct.prototype, "getBundleType", null), __decorate([hvigor_1.TrackAPI], CoreProduct.prototype, "getProductName", null), __decorate([hvigor_1.TrackAPI], CoreProduct.prototype, "getOutputArtifactName", null);
