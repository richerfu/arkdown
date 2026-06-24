/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/project-model-impl.js
 */
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as module_model_impl_js_1 from "../module/module-model-impl";
import * as app_res_model_impl_js_1 from "../res/app-res-model-impl";
import * as core_project_model_impl_js_1 from "./core-project-model-impl";
export class ProjectModelImpl extends core_project_model_impl_js_1.CoreProjectModelImpl {
  constructor(e) {
    super(e), this.appRes = new app_res_model_impl_js_1.AppResModelImpl(path_1.resolve(this.projectPath, common_const_js_1.CommonConst.APP_SCOPE));
  }
  initSubModules() {
    this.project.getSubModules().forEach((e, p) => {
      this.subModels.set(p, new module_model_impl_js_1.ModuleModelImpl(e, this));
    });
  }
  getDefaultBundleName() {
    return this.appRes.getAppResOpt().app ? this.appRes.getAppResOpt().app.bundleName : "";
  }
  getBundleType() {
    var e;
    return this.appRes.getAppResOpt().app && null !== (e = this.appRes.getAppResOpt().app.bundleType) && undefined !== e ? e : common_const_js_1.BundleType.APP;
  }
  getAppRes() {
    return this.appRes;
  }
}
