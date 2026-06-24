/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-gen-lite-source.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_1 from "fs";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import util_1 from "util";
import * as packing_tool_options_js_1 from "../../builder/inner-java-command-builder/packing-tool-options";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as file_util_js_1 from "../../utils/file-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as task_names_js_1 from "../common/task-names";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as legacy_syscap_transform_js_1 from "./legacy-syscap-transform";
export class LegacyGenLiteSource extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.LegacyFATask.GENERATE_LITE_CODE), this._logger = ohos_logger_js_1.OhosLogger.getLogger(LegacyGenLiteSource.name);
  }
  initTaskDepends() {
    var e;
    this.declareDepends("LegacyCompileLiteJS"), this.syscapJsonPath = path_1.resolve(null === (e = this.moduleModel) || undefined === e ? undefined : e.getSourceRootByTargetName(this.targetName), common_const_js_1.CommonConst.SYSCAP_JSON), fs_1.existsSync(this.syscapJsonPath) && this.declareDepends(legacy_syscap_transform_js_1.LegacySyscapTransform.name);
  }
  async doTaskAction() {
    const e = this.service.getRelatedEntryModules(),
      t = this.service.getModuleModel().getModuleType();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(t, e)) for (const t of e) await this.genLiteSource(t);else await this.genLiteSource();
  }
  async genLiteSource(e = "") {
    const t = this.pathInfo,
      s = path_1.resolve(t.getIntermediatesLiteSource(), "assets"),
      o = t.getInterMediatesLoaderOutLitePath();
    await fs_extra_1.copy(o, s);
    const a = path_1.resolve(t.getIntermediatesRes(), e, "config.json"),
      i = path_1.resolve(t.getIntermediatesLiteSource(), e, "config.json");
    if (await fs_extra_1.ensureDir(path_1.resolve(t.getIntermediatesLiteSource(), e)), this.targetData.isSingleDeviceTypeTarget()) await fs_extra_1.copyFile(a, i);else {
      const e = this.targetData.getTargetDeviceType().filter(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e)),
        t = fs_extra_1.readJsonSync(a);
      t.module.deviceType = e, await fs_extra_1.outputJSON(i, t);
    }
    const r = path_1.resolve(t.getIntermediatesRes(), e, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES),
      _ = path_1.resolve(s, this.module.getName(), e, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES);
    await fs_extra_1.ensureDir(path_1.resolve(s, this.module.getName(), e)), await fs_extra_1.copy(r, _), await fs_extra_1.copy(t.getIntermediatesLiteBinSource(), _);
    const n = path_1.resolve(t.getIntermediatesRes(), e, "resources.index"),
      u = path_1.resolve(s, this.module.getName(), e, "resources.index");
    await fs_extra_1.ensureDir(path_1.resolve(s, this.module.getName(), e)), await fs_extra_1.copyFile(n, u);
    const c = path_1.resolve(t.getIntermediatesSysCap(), common_const_js_1.CommonConst.RPCID_SC),
      l = path_1.resolve(t.getIntermediatesLiteSource(), e, common_const_js_1.CommonConst.RPCID_SC);
    fs_extra_1.existsSync(c) && (await fs_extra_1.copyFile(c, l));
    const d = this.targetService.getTargetData();
    if (this.targetService.needPackBin(d)) {
      const e = "generate BIN packaging command",
        s = this.durationEvent.createSubEvent(e, "");
      s.start();
      const o = new packing_tool_options_js_1.PackingToolOptions();
      o.addCalledJarFile(this.sdkInfo.getHapTobin()), o.addProjectPath(t.getIntermediatesLiteSource()).addBinPath(path_1.resolve(t.getModuleBinOutput(), this.targetData.getModuleTargetOutputFileName("", false, build_directory_const_js_1.BuildArtifactExtension.DOT_BIN))), hvigor_1.MetricFactory.createLogEvent(util_1.format(o.build()), hvigor_1.MetricLogType.DEBUG), this._logger.debug(o.build()), file_util_js_1.FileUtil.checkDirWithoutDelete(t.getModuleBinOutput());
      const a = {
        moduleName: this.moduleName,
        commandLine: o.build()
      };
      s.stop(), s.setLog(e, hvigor_1.MetricLogType.INFO);
      const i = "submit BIN packaging task to work pool",
        r = this.durationEvent.createSubEvent(i, "");
      r.start(), await new process_utils_js_1.ProcessUtils(a.moduleName).submitExecutionWithoutReturn(this, this.getWorkerPool(), a, wdk_1.noop, [], r), r.stop(), r.setLog(i, hvigor_1.MetricLogType.INFO);
    }
  }
  async beforeTask() {
    await fs_extra_1.emptydir(this.pathInfo.getIntermediatesLiteSource());
  }
}
