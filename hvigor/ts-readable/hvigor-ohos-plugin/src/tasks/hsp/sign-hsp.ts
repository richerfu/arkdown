/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/sign-hsp.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as file_util_js_1 from "../../utils/file-util";
import * as base_sign_js_1 from "../base/base-sign";
import * as task_names_js_1 from "../common/task-names";
import * as sign_model_js_1 from "../sign/command-builder-impl/sign-model";
import * as package_hsp_js_1 from "./package-hsp";
import * as common_const_js_1 from "../../const/common-const";
export class SignHsp extends base_sign_js_1.BaseSign {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.CommonTask.SIGN_HSP), this.inputFile = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), this.targetData.getModuleTargetOutputFileName("", false, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP)), this.outputFile = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), this.targetData.getModuleTargetOutputFileName("", true, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP));
  }
  declareInputFiles() {
    return super.declareInputFiles().addEntry(this.inputFile, {
      isDirectory: false
    });
  }
  declareOutputFiles() {
    return this.outputFile && super.declareOutputFiles().addEntry(this.outputFile, {
      isDirectory: false
    }), super.declareOutputFiles();
  }
  async doTaskAction() {
    const e = new sign_model_js_1.SignModel(this.signType, this.inputFile, this.outputFile);
    file_util_js_1.FileUtil.deleteFile(e.getOutPutFilePath()), hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.ENABLE_SIGN_TASK) && (await this.signUtil.sign(e, this.durationEvent));
  }
  initTaskDepends() {
    this.declareDepends(package_hsp_js_1.PackageHsp.name);
  }
}
