/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/hvigor-extra-config-info.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as build_directory_const_js_1 from "../const/build-directory-const";
export function getBuildRoot() {
  let o = hvigor_1.hvigorCore.getExtraConfig().get("buildRoot");
  return o || (o = build_directory_const_js_1.BuildDirConst.BUILD_ROOT), o;
}
