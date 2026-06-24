/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/merge-node-assets.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
const ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET;
export class MergeNodeAssets extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.LegacyFATask.MERGE_NODE_ASSETS), this._log = ohos_logger_js_1.OhosLogger.getLogger(MergeNodeAssets.name), this.intermediatesOutPath = this.pathInfo.getInterMediatesLoaderOutPath(), this.intermediatesAssetsPath = path_1.resolve(this.pathInfo.getIntermediatesFaAssetsPath(this.targetData.getProduct()), code_type_enum_js_1.CodeType.JS);
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet(),
      t = this.moduleModel.getSourceSetByTargetName(this.targetName).getCodeMap().get(code_type_enum_js_1.CodeType.ETS),
      s = this.moduleModel.getSourceSetByTargetName(this.targetName).getCodeMap().get(code_type_enum_js_1.CodeType.JS);
    return (t || s) && this.targetData.hasRichDeviceInTarget() && e.addEntry(this.intermediatesOutPath, {
      isDirectory: true
    }), e;
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntry(this.intermediatesAssetsPath, {
      isDirectory: true
    });
  }
  beforeTask() {
    fs_extra_1.removeSync(this.intermediatesAssetsPath);
  }
  doTaskAction() {
    const e = {
        filter: e => !(fs_extra_1.statSync(e).isFile() && e.endsWith(".js"))
      },
      t = path_1.resolve(this.intermediatesOutPath, code_type_enum_js_1.CodeType.ETS);
    fs_extra_1.existsSync(t) && fs_extra_1.copySync(t, this.intermediatesAssetsPath, e);
    const s = path_1.resolve(this.intermediatesOutPath, code_type_enum_js_1.CodeType.JS);
    fs_extra_1.existsSync(s) && fs_extra_1.copySync(s, this.intermediatesAssetsPath, e);
  }
  initTaskDepends() {
    const e = fs_extra_1.existsSync(path_1.resolve(this.sdkInfo.getEtsLoader(), sdk_const_js_1.ToolChainsConst.ROLL_UP_CONFIG_FILE)),
      t = fs_extra_1.existsSync(path_1.resolve(this.sdkInfo.getJsLoader(), sdk_const_js_1.ToolChainsConst.ROLL_UP_CONFIG_FILE));
    this._log.debug(e ? "compile ArkTS with rollup" : "compile ArkTS with webpack"), this._log.debug(t ? "compile ace js with rollup" : "compile ace js with webpack"), this.targetName === ohosTestTargetName ? (this.declareDepends(e ? "LegacyOhosTestCompileArkTS" : "LegacyOhosTestBuildArkTS"), this.declareDepends(t ? "LegacyOhosTestCompileJS" : "LegacyOhosTestBuildJS")) : (this.declareDepends(e ? "LegacyCompileArkTS" : "LegacyBuildArkTS"), this.declareDepends(t ? "LegacyCompileJS" : "LegacyBuildJS"));
  }
}
