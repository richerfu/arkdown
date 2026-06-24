/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-compile-resource.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as compile_resources_util_js_1 from "../../utils/compile-resources-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as abstract_compile_resource_js_1 from "../abstract/abstract-compile-resource";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_hook_compile_resource_js_1 from "./hook/legacy-hook-compile-resource";
import * as legacy_process_resource_js_1 from "./legacy-process-resource";
import * as har_target_util_js_1 from "../../utils/har-target-util";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyCompileResource extends abstract_compile_resource_js_1.AbstractCompileResource {
  constructor(e, t) {
    super(e, (0, legacy_hook_compile_resource_js_1.getTaskRealName)(LegacyFATask.COMPILE_RESOURCE, t)), this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyCompileResource.name), this.entryModuleName = t, this.generateSourceRDir = this.getTaskTempDir(this.targetData, build_directory_const_js_1.BuildDirConst.R, false), this.resConfigFilePath = path_1.resolve(this.pathInfo.getIntermediatesRes(), null != t ? t : "", common_const_js_1.CommonConst.RES_CONFIG_FILE), this.processProfileTask = (0, legacy_hook_compile_resource_js_1.getTaskRealName)({
      name: legacy_process_resource_js_1.LegacyProcessResource.name
    }, this.entryModuleName).name;
  }
  declareExecutionCommand() {
    return this.getCommand().toString();
  }
  declareInputFiles() {
    return this.restoolConfigBuilder.inputFiles.addEntries([this.resConfigFilePath], {
      isDirectory: false
    });
  }
  initTaskDepends() {
    this.declareDepends((0, legacy_hook_compile_resource_js_1.getTaskRealName)({
      name: legacy_process_resource_js_1.LegacyProcessResource.name
    }, this.entryModuleName).name);
  }
  initHarModuleDepends() {
    har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).forEach((e, t) => this.declareDepends(`${e}@${LegacyFATask.COMPILE_RESOURCE.name}`, t));
  }
  async doTaskAction() {
    await this.compilationProcess(compile_resources_util_js_1.CompileResourceBuildCommandType.FILE, this.targetData);
  }
  async invokeRestool(e) {
    undefined === this.entryModuleName ? await this.executeCommand(this.getCommand(), this._log, () => {
      var t, s;
      if (this.targetData.isSingleDeviceTypeTarget()) return;
      const o = this.targetData.getTargetDeviceType().filter(e => !device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e)),
        r = this.targetData.getTargetDeviceType().filter(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e)),
        a = path_1.resolve(this.pathInfo.getIntermediatesRes(), null !== (t = this.entryModuleName) && undefined !== t ? t : "", common_const_js_1.CommonConst.CONFIG_JSON),
        i = fs_extra_1.readJsonSync(a);
      if (i.module.deviceType = o, fs_extra_1.outputJSONSync(a, i, {
        spaces: "\t"
      }), this.targetService.needPackBin(e)) return;
      const c = null !== (s = this.entryModuleName) && undefined !== s ? s : "",
        _ = path_1.resolve(this.pathInfo.getIntermediatesRes(), c, device_type_const_js_1.DeviceTypeConst.LITE, common_const_js_1.CommonConst.CONFIG_JSON);
      i.module.deviceType = r, fs_extra_1.ensureDirSync(path_1.resolve(this.pathInfo.getIntermediatesLiteSource(), c, device_type_const_js_1.DeviceTypeConst.LITE)), fs_extra_1.outputJSONSync(_, i, {
        spaces: "\t"
      });
    }) : await this.executeCommand(this.getCommand(), this._log, async () => {
      await this.retainDeviceType(e, this.entryModuleName, this._log);
    });
  }
}
