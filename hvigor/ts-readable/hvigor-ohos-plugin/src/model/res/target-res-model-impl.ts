/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/res/target-res-model-impl.js
 */
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as json_util_js_1 from "../../utils/json-util";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as core_res_model_impl_js_1 from "./core-res-model-impl";
export class ModuleTargetResImpl extends core_res_model_impl_js_1.CoreResModelImpl {
  constructor(e) {
    super(path_1.resolve(e, common_const_js_1.CommonConst.MODULE_JSON5), path_1.resolve(e, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES));
  }
  getModuleJsonOpt() {
    return res_model_loader_js_1.resModelLoader.getModuleJson(this.getJsonPath());
  }
  getFormJsonOpt(e) {
    return json_util_js_1.JsonUtil.getJson5Obj(path_1.resolve(this.getResourcePath(), e));
  }
}
