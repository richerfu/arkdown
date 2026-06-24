/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/ohos-har-task.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as oh_package_loader_1 from "../../utils/loader/file/oh-package-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as har_extend_info_js_1 from "../har/har-extend-info";
import * as ohos_module_task_js_1 from "./ohos-module-task";
const DEFAULT_MAIN_FILED_FILE_NAME = "index";
export const DEFAULT_MAIN_FILED_FILE_SUFFIX = [".ets", ".ts", ".js"];
const _log = ohos_logger_js_1.OhosLogger.getLogger("process-package-json");
export class OhosHarTask extends ohos_module_task_js_1.OhosModuleTask {
  constructor(e, o) {
    if (super(e, o), this._harExtendInfo = new har_extend_info_js_1.HarExtendInfo(this.moduleModel, e.getBuildOption()), this.modulePackageJsonPath = this.getModulePackageJsonPath(), !this.packageJsonObj) {
      const e = oh_package_loader_1.ohPackageLoader.getNodeOhPackagePath(this.isOhpmProject ? common_const_js_1.CommonConst.OH_PACKAGE_JSON5 : common_const_js_1.CommonConst.PACKAGE_JSON);
      _log.printErrorExit("THE_%S_FILE_DOES_NOT_EXIST", [e, this.modulePackageJsonPath], [[e]]);
    }
  }
  getModulePackageJsonPath() {
    return this.moduleModel.isOhpmProject() ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath();
  }
  getPackageJsonValidMainField() {
    const e = this.packageJsonObj.main;
    let o;
    if (undefined === e || "" === e) o = this.findValidMainField();else {
      o = e;
      const t = path_1.resolve(this.moduleModel.getProjectDir(), o);
      fs_extra_1.existsSync(t) || _log.printErrorExit("THE_MAIN_FIELD_SET_IN_PACKAGE_JSON_CORRSPONDING_PATH", [t, this.modulePackageJsonPath]);
    }
    return o;
  }
  findValidMainField() {
    const e = this.moduleModel.getProjectDir();
    let o;
    return DEFAULT_MAIN_FILED_FILE_SUFFIX.forEach(t => {
      const s = `index${t}`,
        _ = path_1.resolve(e, `index${t}`);
      fs_extra_1.existsSync(_) && (o = s);
    }), null != o ? o : "index.ets";
  }
}
