/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-sign-lite-bin.js
 */
import * as path from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as sign_type_enum_js_1 from "../../enum/sign-type-enum";
import * as base_sign_js_1 from "../base/base-sign";
import * as task_names_js_1 from "../common/task-names";
import * as sign_model_js_1 from "../sign/command-builder-impl/sign-model";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacySignLiteBin extends base_sign_js_1.BaseSign {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.LegacyFATask.SIGN_LITE_BIN, sign_type_enum_js_1.SignTypeEnum.BIN), this.inputFile = path.resolve(this.pathInfo.getModuleBinOutput(), this.targetData.getModuleTargetOutputFileName("", false, build_directory_const_js_1.BuildArtifactExtension.DOT_BIN)), this.outputFile = path.resolve(this.pathInfo.getModuleBinOutput(), this.targetData.getModuleTargetOutputFileName("", true, build_directory_const_js_1.BuildArtifactExtension.DOT_BIN)), this.targetServiceData = this.targetService.getTargetData();
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.targetService.needPackBin(this.targetServiceData) ? e.addEntry(this.inputFile, {
      isDirectory: false
    }) : e;
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    return this.outputFile && e.addEntry(this.outputFile, {
      isDirectory: false
    }), e;
  }
  taskShouldDo() {
    return this.targetService.needPackBin(this.targetServiceData);
  }
  initTaskDepends() {
    this.declareDepends(LegacyFATask.GENERATE_LITE_CODE.name);
  }
  async doTaskAction() {
    await this.signUtil.sign(new sign_model_js_1.SignModel(sign_type_enum_js_1.SignTypeEnum.BIN, this.inputFile, this.outputFile), this.durationEvent);
  }
}
