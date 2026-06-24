/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/process-package-json.js
 */
import * as fse from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as task_names_js_1 from "../common/task-names";
import * as core_process_package_json_js_1 from "./core-process-package-json";
import * as har_extend_info_js_1 from "./har-extend-info";
var Task = task_names_js_1.TaskNames.Task;
export class ProcessPackageJson extends core_process_package_json_js_1.CoreProcessPackageJson {
  constructor(e) {
    super(e, Task.PROCESS_PACKAGE_JSON), this.generatePackageJsonFilePath = path_1.resolve(this.generatePmDir, common_const_js_1.CommonConst.PACKAGE_JSON);
  }
  doTaskAction() {
    super.doTaskAction(), fse.outputJSONSync(this.generatePackageJsonFilePath, this.packageJsonObj);
  }
  addArtifactType(e) {
    e.ohos.artifactType = har_extend_info_js_1.ArtifactType.OBFUSCATION;
  }
}
