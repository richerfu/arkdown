/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-app.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../const/common-const";
import * as sign_type_enum_js_1 from "../enum/sign-type-enum";
import * as file_util_js_1 from "../utils/file-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as package_app_js_1 from "./package-app";
import * as sign_packages_from_app_js_1 from "./sign-packages-from-app";
import * as sign_model_js_1 from "./sign/command-builder-impl/sign-model";
import * as sign_util_js_1 from "./sign/sign-util";
import * as ohos_app_task_js_1 from "./task/ohos-app-task";
export class SignApp extends ohos_app_task_js_1.OhosAppTask {
  constructor(t, e) {
    super(t, e, task_names_js_1.TaskNames.CommonTask.SIGN_APP), this.log = ohos_logger_js_1.OhosLogger.getLoggerWithDurationId(SignApp.name, this.durationEvent.getId()), this.pathInfo = this.service.getPathInfo();
  }
  get appWithSignedPkg() {
    var t, e;
    return null !== (e = null === (t = this.service.getBuildOption().packOptions) || undefined === t ? undefined : t.appWithSignedPkg) && undefined !== e && e;
  }
  async beforeAlwaysAction() {
    var t;
    this.signUtil = new sign_util_js_1.SignUtil(this.service, sign_type_enum_js_1.SignTypeEnum.APP, this.pathInfo, this.service.getTargetProduct(), this.service.getSdkInfo()), this.signingConfig = this.signUtil._signingConfig, this.log.debug("signingConfigName of signApp is : ", null === (t = this.signUtil._signingConfig) || undefined === t ? undefined : t.name), this.log.warn(this.signUtil._signingConfigCheckLogStr);
  }
  declareInputs() {
    return sign_util_js_1.SignUtil.getSigningConfigInputs(this.service.getSdkInfo(), this.signingConfig).set("appWithSignedPkg", this.appWithSignedPkg);
  }
  declareInputFiles() {
    if (this.signUtil) {
      const t = this.signUtil.getSigningConfigInputFiles().addEntry(path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAppOutputFileName()), {
          isDirectory: false
        }),
        e = path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAllAppOutputFileName());
      return fs_extra_1.existsSync(e) && t.addEntry(path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAllAppOutputFileName()), {
        isDirectory: false
      }), t;
    }
    return super.declareInputFiles();
  }
  declareOutputFiles() {
    const t = super.declareOutputFiles(),
      e = path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAppOutputFileName(true));
    if (e && t.addEntry(e, {
      isDirectory: false
    }), this.appWithSignedPkg) {
      const e = path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAllAppOutputFileName(true));
      t.addEntry(e, {
        isDirectory: false
      });
    }
    return t;
  }
  async doTaskAction() {
    const t = path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAppOutputFileName(true)),
      e = path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAllAppOutputFileName(true)),
      i = new sign_model_js_1.SignModel(sign_type_enum_js_1.SignTypeEnum.APP, path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAppOutputFileName()), t),
      s = new sign_model_js_1.SignModel(sign_type_enum_js_1.SignTypeEnum.APP, path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAllAppOutputFileName()), e);
    file_util_js_1.FileUtil.deleteFile(i.getOutPutFilePath()), file_util_js_1.FileUtil.deleteFile(s.getOutPutFilePath()), hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.ENABLE_SIGN_TASK) && (await this.signUtil.sign(i, this.durationEvent), this.appWithSignedPkg && (await this.signUtil.sign(s, this.durationEvent)));
  }
  initTaskDepends() {
    this.dependsOn(package_app_js_1.PackageApp.name), this.dependsOn(sign_packages_from_app_js_1.SignPackagesFromApp.name);
  }
}
