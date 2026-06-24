/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/resolver/ohpm-package-resolver.js
 */
import * as common_const_js_1 from "../../const/common-const";
import * as base_package_resolver_js_1 from "./base-package-resolver";
export class OhpmPackageResolver extends base_package_resolver_js_1.BasePackageResolver {
  constructor() {
    super(common_const_js_1.CommonConst.OH_PACKAGE_JSON5, common_const_js_1.CommonConst.OH_MODULES);
  }
}
