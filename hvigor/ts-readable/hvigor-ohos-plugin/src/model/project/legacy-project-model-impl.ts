/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/legacy-project-model-impl.js
 */
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as legacy_module_model_impl_js_1 from "../module/legacy-module-model-impl";
import * as core_project_model_impl_js_1 from "./core-project-model-impl";
import * as common_const_js_1 from "../../const/common-const";
export class LegacyProjectModelImpl extends core_project_model_impl_js_1.CoreProjectModelImpl {
  constructor(e) {
    super(e);
  }
  initSubModules() {
    this.project.getSubModules().forEach((e, o) => {
      this.subModels.set(o, new legacy_module_model_impl_js_1.LegacyModuleModelImpl(e, this));
    });
  }
  getDefaultBundleName() {
    let e = "";
    try {
      this.subModels.forEach(o => {
        if (o.getModuleType() === module_type_enum_js_1.ModuleType.Entry) {
          throw e = o.getSourceSetByTargetName().getLegacyModuleTargetRes().getConfigJsonOpt().app.bundleName, new Error("End loop.");
        }
      });
    } catch (e) {}
    return e;
  }
  getBundleType() {
    for (const [, e] of this.subModels.entries()) if (e.getModuleType() === module_type_enum_js_1.ModuleType.Entry) {
      return e.isInstallFree() ? common_const_js_1.BundleType.ATOMIC_SERVICE : common_const_js_1.BundleType.APP;
    }
    return common_const_js_1.BundleType.APP;
  }
  isFaMode() {
    return true;
  }
}
