/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/module-build-profile-loader.js
 */
import * as common_const_js_1 from "../../../const/common-const";
import * as abstract_build_profile_loader_js_1 from "./abstract-build-profile-loader";
class ModuleBuildProfileLoader extends abstract_build_profile_loader_js_1.AbstractBuildProfileLoader {
  getBuildProfile(e) {
    return this.getBuildProfileJson5Obj(e, false);
  }
  setBuildProfileJson5Obj(e, o, r = true) {
    super.setBuildProfileJson5ObjCheck(e, o, r ? common_const_js_1.BuildProfileSchemaFileConst.HAP_MODULE_BUILD_PROFILE_SCHEMA_PATH : undefined);
  }
}
export const moduleBuildProfileLoader = new ModuleBuildProfileLoader();
