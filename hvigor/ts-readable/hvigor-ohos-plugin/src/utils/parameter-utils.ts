/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/parameter-utils.js
 */
import * as common_const_js_1 from "../const/common-const";
import * as hvigor_1 from "@ohos/hvigor";
export function getPropertyKeepDependency() {
  return hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.KEEP_DEPENDENCY);
}
