/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/har-build-profile-task.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as task_names_js_1 from "../common/task-names";
import * as task_util_js_1 from "../../utils/task-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_har_task_js_1 from "../task/ohos-har-task";
import * as build_profile_utils_js_1 from "../../utils/build-profile-utils";
var __decorate = this && this.__decorate || function (e, t, r, i) {
  var o,
    a = arguments.length,
    s = a < 3 ? t : null === i ? i = Object.getOwnPropertyDescriptor(t, r) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) s = Reflect.decorate(e, t, r, i);else for (var l = e.length - 1; l >= 0; l--) (o = e[l]) && (s = (a < 3 ? o(s) : a > 3 ? o(t, r, s) : o(t, r)) || s);
  return a > 3 && s && Object.defineProperty(t, r, s), s;
};
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class HarBuildProfileTask extends ohos_har_task_js_1.OhosHarTask {
  constructor(e) {
    super(e, CommonTask.CREATE_HAR_BUILD_PROFILE), this._log = ohos_logger_js_1.OhosLogger.getLogger(HarBuildProfileTask.name), this.harTaskService = e, this.initDefaultData();
  }
  initDefaultData() {
    this.harBuildProfileData = {
      HAR_VERSION: this.harVersion,
      BUILD_MODE_NAME: this.buildModeName,
      DEBUG: this.harTaskService.isDebug(),
      TARGET_NAME: this.harTargetName
    };
  }
  get appJsonPath() {
    return this.service.getProjectModel().getAppRes().getJsonPath();
  }
  get projectProfilePath() {
    return this.projectModel.getProfilePath();
  }
  get harBuildProfilePath() {
    return this.pathInfo.getHarGenerateBuildProfilePath();
  }
  get buildModeName() {
    return (0, task_util_js_1.getBuildModeName)();
  }
  get buildMode() {
    return this.targetService.getBuildMode();
  }
  get buildProfileFields() {
    return this.targetService.getBuildProfileFields();
  }
  get harVersion() {
    return this.packageJsonObj.version;
  }
  get harTargetName() {
    return this.harTaskService.getTargetData().getTargetName();
  }
  initTaskDepends() {}
  doTaskAction() {
    const e = this.buildProfileFields,
      t = e ? {
        ...this.harBuildProfileData,
        ...e
      } : {
        ...this.harBuildProfileData
      };
    hvigor_1.LocalFileWriter.getInstance().writeStr(this.harBuildProfilePath, (0, build_profile_utils_js_1.normalizedFileData)(t, this.targetData, this._log));
  }
}
__decorate([(0, hvigor_1.InputFile)()], HarBuildProfileTask.prototype, "appJsonPath", null), __decorate([(0, hvigor_1.InputFile)()], HarBuildProfileTask.prototype, "projectProfilePath", null), __decorate([(0, hvigor_1.OutputFile)()], HarBuildProfileTask.prototype, "harBuildProfilePath", null), __decorate([(0, hvigor_1.Input)()], HarBuildProfileTask.prototype, "buildModeName", null), __decorate([(0, hvigor_1.Input)()], HarBuildProfileTask.prototype, "buildMode", null), __decorate([(0, hvigor_1.Input)()], HarBuildProfileTask.prototype, "buildProfileFields", null), __decorate([(0, hvigor_1.Input)()], HarBuildProfileTask.prototype, "harVersion", null), __decorate([(0, hvigor_1.Input)()], HarBuildProfileTask.prototype, "harTargetName", null);
