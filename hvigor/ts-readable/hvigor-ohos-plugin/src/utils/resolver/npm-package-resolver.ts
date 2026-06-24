/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/resolver/npm-package-resolver.js
 */
import * as common_const_js_1 from "../../const/common-const";
import * as base_package_resolver_js_1 from "./base-package-resolver";
export class NpmPackageResolver extends base_package_resolver_js_1.BasePackageResolver {
  constructor() {
    super(common_const_js_1.CommonConst.PACKAGE_JSON, common_const_js_1.CommonNodeConst.NODE_MODULES);
  }
}
