/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/ark-linter.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_1 from "fs";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../utils/task-util";
import * as abstract_compile_node_js_1 from "../abstract/abstract-compile-node";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_pre_build_js_1 from "../legacy-tasks/legacy-pre-build";
import * as legacy_process_profile_js_1 from "../legacy-tasks/legacy-process-profile";
import * as pre_build_js_1 from "../pre-build";
import * as run_ark_js_1 from "../worker/run-ark";
import * as fse from "fs-extra";
import * as sdk_const_js_1 from "../../const/sdk-const";
var CommonTask = task_names_js_1.TaskNames.CommonTask,
  Task = task_names_js_1.TaskNames.Task;
export class ArkLinter extends abstract_compile_node_js_1.AbstractCompileNode {
  constructor(e) {
    super(e, code_type_enum_js_1.CodeType.ETS, Task.LINT), this._log = ohos_logger_js_1.OhosLogger.getLogger(ArkLinter.name);
  }
  taskShouldDo() {
    return super.taskShouldDo() && this.codeType === code_type_enum_js_1.CodeType.ETS && this.sdkInfo.hasRollUpPluginInEtsLoader;
  }
  async doTaskAction() {
    const e = "initialize and add necessary parameters",
      t = this.durationEvent.createSubEvent(e, "");
    t.start();
    const s = await this.findAvailableEntry(),
      r = await this.customizeProjectConfig(),
      o = {};
    o[path_1.relative(r.aceModuleRoot, s)] = s, this.service.getDependencyMainPaths(), t.stop(), t.setLog(e, hvigor_1.MetricLogType.INFO);
    const i = "submit ark linter task to work pool",
      a = this.durationEvent.createSubEvent(i, "");
    a.start(), await (0, run_ark_js_1.submitArkLintWork)(this, this.getWorkerPool(), this._log, r, o, a), a.stop(), a.setLog(i, hvigor_1.MetricLogType.INFO);
  }
  async findAvailableEntry() {
    const e = await hvigor_1.Json5Reader.readJson5File(this.packageManagerPath),
      t = this.moduleModel.getProjectDir();
    if (!e.main) {
      const e = ["ets", "ts", "js"].find(e => fs_1.existsSync(path_1.resolve(t, `index.${e}`)));
      return e || this._log._buildError("No available main file defined in oh-package.json.")._file(this.moduleModel.getOhPackageJson5Path())._printErrorAndExit(this.moduleName), path_1.resolve(t, `index.${e}`);
    }
    return e.main && !fs_1.existsSync(path_1.resolve(t, e.main)) && this._log._buildError(`Invalid main file ${e.main} defined in oh-package.json.`)._file(this.moduleModel.getOhPackageJson5Path())._printErrorAndExit(this.moduleName), path_1.resolve(t, e.main);
  }
  async customizeProjectConfig() {
    const e = await this.initDefaultArkCompileConfig();
    process.env.compileTool = common_const_js_1.ArkPackConst.COMPILE_TOOL, e.projectPath = e.projectTopDir;
    const t = {};
    return this.projectModel.getSubModuleModels().forEach((e, s) => t[s] = {
      belongProjectPath: e.getProjectDir(),
      moduleName: s,
      modulePkgPath: e.getProjectDir()
    }), e.projectModel = t, e;
  }
  initTaskDepends() {
    this.declareDepends(this.isFaMode ? legacy_pre_build_js_1.LegacyPreBuild.name : pre_build_js_1.PreBuild.name), this.declareDepends(legacy_process_profile_js_1.LegacyProcessProfile.name), (0, task_util_js_1.limitMinApiVersion)(this.targetData, sdk_const_js_1.ApiVersion.API_VERSION_9) && !this.isFaMode && this.declareDepends(CommonTask.CREATE_HAR_BUILD_PROFILE.name);
  }
  declareInputs() {
    var e;
    return new Map().set("customTypes", null !== (e = this.targetService.getCustomTypes()) && undefined !== e ? e : "").set(common_const_js_1.CommonConst.BUILD_MODE, this.targetService.getBuildMode());
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet();
    this.sourcePath && fs_1.existsSync(this.sourcePath) && e.addEntry(this.sourcePath, {
      isDirectory: true
    });
    const t = this.pathInfo.getHarGenerateBuildProfilePath();
    return fse.existsSync(t) && e.addEntry(t), e;
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet();
  }
}
