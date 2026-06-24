/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/packing-check/packing-check-har.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../../const/common-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as ohos_har_task_js_1 from "../task/ohos-har-task";
import * as packing_check_util_js_1 from "../../utils/validate/packing-check-util";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
var Task = task_names_js_1.TaskNames.Task;
export class PackingCheckHar extends ohos_har_task_js_1.OhosHarTask {
  constructor(e) {
    var t, i;
    super(e, Task.PACKING_CHECK_HAR), this._log = ohos_logger_js_1.OhosLogger.getLogger(task_names_js_1.TaskNames.Task.PACKING_CHECK_HAR.name);
    const s = e.getTargetData().getTargetName();
    this.moduleRoot = this.targetService.getTargetData().getPathInfo().getModulePath(), this.releaseWhiteListTemDir = path_1.resolve(this.moduleRoot, common_const_js_1.CommonConst.RELEASE_HAR_WHITE_LIST_DIR_NAME), this.byteCodeHarAbcOutputDir = path_1.resolve(this.pathInfo.getInterMediatesLoaderOutPath(), code_type_enum_js_1.CodeType.ETS), this.originalEtsPath = null !== (i = null === (t = this.moduleModel.getSourceSetByTargetName(s).getCodeMap().get(code_type_enum_js_1.CodeType.ETS)) || undefined === t ? undefined : t.getSrcPath()) && undefined !== i ? i : "", this.packageHarTempDir = this.getTaskTempDir(this.targetData, `${this.targetService.getTargetData().getTargetName()}@PackageHar`, false);
  }
  taskShouldDo() {
    var e, t;
    const i = this._harExtendInfo.isObfuscatedHar() || this.targetService.isByteCodeHar();
    return (null === (t = null === (e = this.targetService.getBuildOption().packOptions) || undefined === e ? undefined : e.enableSourceCodeCheck) || undefined === t || t) && i;
  }
  doTaskAction() {
    const e = [];
    e.push(...packing_check_util_js_1.PackingCheckUtil.recoverOriginalPath(packing_check_util_js_1.PackingCheckUtil.collectSourceCodeFiles(this.releaseWhiteListTemDir), this.releaseWhiteListTemDir));
    const t = [".c", ".h", ".cpp", ".cs", ".java", ".rs", ".py", ".go", ".ets", ".ts"],
      i = e => t.some(t => e.endsWith(t));
    packing_check_util_js_1.PackingCheckUtil.getResourcePaths(this.targetService).forEach(t => {
      const s = path_1.resolve(this.packageHarTempDir, path_1.relative(this.moduleRoot, t)),
        o = packing_check_util_js_1.PackingCheckUtil.recoverPackagedFiles(packing_check_util_js_1.PackingCheckUtil.collectSourceCodeFiles(t, i), s);
      e.push(...packing_check_util_js_1.PackingCheckUtil.recoverOriginalPath(o, t));
    }), this.targetService.isByteCodeHar() ? this.collectSourceCodeFilesForByteCodeHar(e) : this._harExtendInfo.isObfuscatedHar() && this.collectSourceCodeFilesForObfuscatedHar(e);
    const s = (0, hvigor_common_1.getOsLanguage)(),
      o = `${(0, hvigor_logger_1.getUrl)(hvigor_logger_1.DEVELOPER_URL)}/consumer/${s}/doc/harmonyos-guides/ide-hvigor-build-profile-app#section03812484215`;
    0 !== e.length && this._log.warn(`Unexpected source code files packaged in '${this.moduleName}'. Please remove them if not needed. Reference: ${o}\nIf the checking is not needed, you can set buildOption/packOptions/enableSourceCodeCheck to false at file: ${this.projectModel.getProfilePath()}\n${e.join("\n")}`), packing_check_util_js_1.PackingCheckUtil.clearResourceFilesCache();
  }
  initTaskDepends() {}
  collectSourceCodeFilesForByteCodeHar(e) {
    const t = packing_check_util_js_1.PackingCheckUtil.collectSourceCodeFiles(this.byteCodeHarAbcOutputDir);
    e.push(...packing_check_util_js_1.PackingCheckUtil.recoverOriginalPath(t, this.originalEtsPath));
  }
  collectSourceCodeFilesForObfuscatedHar(e) {
    if (!fs_extra_1.existsSync(this.pathInfo.getInterMediatesLoaderOutPath())) return;
    const t = [".c", ".h", ".cpp", ".cs", ".java", ".rs", ".py", ".go", ".ets"],
      i = packing_check_util_js_1.PackingCheckUtil.collectSourceCodeFiles(this.pathInfo.getInterMediatesLoaderOutPath(), e => t.some(t => e.endsWith(t)) && !e.endsWith(".d.ets"));
    e.push(...packing_check_util_js_1.PackingCheckUtil.recoverOriginalPath(i, this.originalEtsPath));
  }
}
