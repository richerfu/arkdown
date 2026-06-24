/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/har/legacy-process-oh-package-json.js
 */
import * as fse from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../../const/common-const";
import * as task_names_js_1 from "../../common/task-names";
import * as process_oh_package_json_js_1 from "../../har/process-oh-package-json";
import * as ohos_har_task_js_1 from "../../task/ohos-har-task";
import * as legacy_pre_build_js_1 from "../legacy-pre-build";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyProcessOHPackageJson extends ohos_har_task_js_1.OhosHarTask {
  constructor(e) {
    super(e, LegacyFATask.PROCESS_OH_PACKAGE_JSON), this.generateOhPackageJsonFilePath = path_1.resolve(this.pathInfo.getGeneratePmDir(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
  }
  doTaskAction() {
    (0, process_oh_package_json_js_1.processResourceConfig)(this.targetService, this.packageJsonObj), fse.outputJSONSync(this.generateOhPackageJsonFilePath, this.packageJsonObj);
  }
  initTaskDepends() {
    this.declareDepends(legacy_pre_build_js_1.LegacyPreBuild.name);
  }
}
