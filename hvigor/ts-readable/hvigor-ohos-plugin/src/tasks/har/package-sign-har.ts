/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/package-sign-har.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as sign_type_enum_js_1 from "../../enum/sign-type-enum";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as npm_utils_js_1 from "../../utils/npm-utils";
import * as base_sign_js_1 from "../base/base-sign";
import * as task_names_js_1 from "../common/task-names";
import * as sign_model_js_1 from "../sign/command-builder-impl/sign-model";
import * as sign_util_js_1 from "../sign/sign-util";
export class PackageSignHar extends base_sign_js_1.BaseSign {
  constructor(t) {
    var s;
    super(t, task_names_js_1.TaskNames.CommonTask.PACKAGE_SIGN_HAR, sign_type_enum_js_1.SignTypeEnum.HAR), this.compiledRoot = this.getTaskTempDir(this.targetData, `${this.targetData.getTargetName()}@${task_names_js_1.TaskNames.Task.PACKAGE_HAR.name}`, false), this.taskTmpRoot = this.getTaskTempDir(this.targetData), this.packageRoot = path_1.resolve(this.taskTmpRoot, build_directory_const_js_1.BuildDirConst.PACKAGE_HAP);
    const e = null === (s = this.targetData.getTargetOpt().output) || undefined === s ? undefined : s.artifactName,
      i = e ? `${e}` : `${this.moduleName}`;
    this.outputFile = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), `${i}-signed.har`);
  }
  beforeTask() {
    fs_extra_1.emptyDirSync(this.taskTmpRoot), fs_extra_1.existsSync(this.outputFile) && fs_extra_1.rmSync(this.outputFile);
  }
  declareInputs() {
    const t = super.declareInputs();
    return t.delete(common_const_js_1.CommonConst.ENABLE_SIGN_TASK), t.set("isDaemon", inject_util_js_1.InjectUtil.isDaemon()).set("compileApiVersion", this.compileApiVersion).set("compatibleApiVersion", this.compatibleApiVersion).set("isSigned", (0, sign_util_js_1.isSigned)(this.projectModel, true)).set("isHarmonyOS", this.targetService.getTargetData().isHarmonyOS()), t;
  }
  taskShouldDo() {
    return inject_util_js_1.InjectUtil.isDaemon() && this.isOhpmProject && (0, sign_util_js_1.isSigned)(this.projectModel, true) && this.compileApiVersion >= sdk_const_js_1.ApiVersion.API_VERSION_12 && this.compatibleApiVersion >= sdk_const_js_1.ApiVersion.API_VERSION_12 && this.targetService.getTargetData().isHarmonyOS() && fs_extra_1.existsSync(this.sdkInfo.getHosSignTool());
  }
  declareInputFiles() {
    return super.declareInputFiles().addEntry(this.compiledRoot, {
      isDirectory: true
    }).addEntry(this.taskTmpRoot, {
      isDirectory: true
    });
  }
  declareOutputFiles() {
    const t = super.declareOutputFiles();
    return t.addEntry(this.outputFile, {
      isDirectory: false
    }), t;
  }
  async doTaskAction() {
    fs_extra_1.copySync(this.compiledRoot, this.packageRoot);
    const t = new sign_model_js_1.SignModel(this.signType, this.taskTmpRoot, "");
    await this.signUtil.sign(t, this.durationEvent), await (0, npm_utils_js_1.execOhpmPack)(this.taskTmpRoot, this.outputFile, this.durationEvent, false);
  }
  initTaskDepends() {}
}
