/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/packing-check/packing-check.js
 */
import path_1 from "path";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as packing_check_util_js_1 from "../../utils/validate/packing-check-util";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import fs_1 from "fs";
var Task = task_names_js_1.TaskNames.Task;
export class PackingCheck extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    var t, s;
    super(e, Task.PACKING_CHECK), this._log = ohos_logger_js_1.OhosLogger.getLogger(Task.PACKING_CHECK.name);
    const i = this.targetService.getTargetData().getPathInfo(),
      o = e.getTargetData().getTargetName(),
      a = e.getTargetData().getModuleSourceSetModel(),
      r = a.getModuleTargetRes();
    this.moduleJsonObj = a.getModuleTargetRes().getModuleJsonOpt(), this.targetJsonPath = r.getJsonPath(), this.etsAssetsPath = path_1.resolve(i.getInterMediatesLoaderOutPath(), code_type_enum_js_1.CodeType.ETS), this.etsOriginalPath = null !== (s = null === (t = this.moduleModel.getSourceSetByTargetName(o).getCodeMap().get(code_type_enum_js_1.CodeType.ETS)) || undefined === t ? undefined : t.getSrcPath()) && undefined !== s ? s : "", this.intermediateResourceDir = path_1.resolve(this.pathInfo.getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES);
  }
  taskShouldDo() {
    var e, t;
    return null === (t = null === (e = this.targetService.getBuildOption().packOptions) || undefined === e ? undefined : e.enableSourceCodeCheck) || undefined === t || t;
  }
  doTaskAction() {
    const e = [],
      t = packing_check_util_js_1.PackingCheckUtil.collectSourceCodeFiles(this.etsAssetsPath);
    e.push(...packing_check_util_js_1.PackingCheckUtil.recoverOriginalPath(t, this.etsOriginalPath)), this.checkSoFilesExist();
    const s = [".c", ".h", ".cpp", ".cs", ".java", ".rs", ".py", ".go", ".ets", ".ts"],
      i = e => s.some(t => e.endsWith(t));
    packing_check_util_js_1.PackingCheckUtil.getResourcePaths(this.targetService).forEach(t => {
      const s = packing_check_util_js_1.PackingCheckUtil.collectSourceCodeFiles(t, i);
      e.push(...packing_check_util_js_1.PackingCheckUtil.recoverOriginalPath(packing_check_util_js_1.PackingCheckUtil.recoverPackagedFiles(s, this.intermediateResourceDir), t));
    });
    const o = (0, hvigor_common_1.getOsLanguage)(),
      a = `${(0, hvigor_logger_1.getUrl)(hvigor_logger_1.DEVELOPER_URL)}/consumer/${o}/doc/harmonyos-guides/ide-hvigor-build-profile-app#section03812484215`;
    0 !== e.length && this._log.warn(`Unexpected source code files packaged in '${this.moduleName}'. Please remove them if not needed. Reference: ${a}\nIf the checking is not needed, you can set buildOption/packOptions/enableSourceCodeCheck to false at file: ${this.projectModel.getProfilePath()}\n${e.join("\n")}`), packing_check_util_js_1.PackingCheckUtil.clearResourceFilesCache();
  }
  checkSoFilesExist() {
    var e, t;
    const s = this.moduleJsonObj.module.extensionAbilities;
    if (!(null == s ? undefined : s.length)) return;
    const i = null !== (t = null === (e = this.targetService.getBuildOption().externalNativeOptions) || undefined === e ? undefined : e.abiFilters) && undefined !== t ? t : ["arm64-v8a"],
      o = this.pathInfo.getIntermediatesStrippedLibsDir();
    for (const e of s) {
      const t = e.srcEntry;
      if (!t || !t.endsWith(".so")) continue;
      i.every(e => fs_1.existsSync(path_1.join(o, e, t))) || this._log.printErrorExit("MODULE_SRC_ENTRY_NOT_FOUND", [t, this.targetJsonPath]);
    }
  }
  initTaskDepends() {}
}
