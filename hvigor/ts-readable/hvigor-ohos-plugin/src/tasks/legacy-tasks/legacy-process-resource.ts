/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-process-resource.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as abstract_process_resource_js_1 from "../abstract/abstract-process-resource";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_hook_compile_resource_js_1 from "./hook/legacy-hook-compile-resource";
import fs_1 from "fs";
import * as legacy_process_oh_package_json_js_1 from "./har/legacy-process-oh-package-json";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
const ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET;
export class LegacyProcessResource extends abstract_process_resource_js_1.AbstractProcessResource {
  constructor(e, s) {
    var t;
    super(e, (0, legacy_hook_compile_resource_js_1.getTaskRealName)(LegacyFATask.PROCESS_RESOURCE, s)), this.entryModuleName = s, this.outputDir = path_1.resolve(this.pathInfo.getIntermediatesRes(), null !== (t = this.entryModuleName) && undefined !== t ? t : ""), this.resConfigFilePath = path_1.resolve(this.outputDir, common_const_js_1.CommonConst.RES_CONFIG_FILE);
  }
  initTaskDependsForOtherModule() {
    this.featureAddEntryDpends();
  }
  initTaskDepends() {
    super.initTaskDepends(), this.targetService.getTargetData().getModuleModel().isHarModule() && this.declareDepends(legacy_process_oh_package_json_js_1.LegacyProcessOHPackageJson.name);
  }
  featureAddEntryDpends() {
    if (this.service.getModuleModel().getModuleType() !== module_type_enum_js_1.ModuleType.Feature) return;
    const e = this.service.getRelatedEntryModules();
    undefined !== e && e.forEach(e => {
      var s;
      const t = this.targetData.findTargetDataByName(e);
      if (!t || this.targetName === ohosTestTargetName) return;
      const o = `${t.getTargetName()}@${LegacyFATask.COMPILE_RESOURCE.name}`;
      (null === (s = this.node.findModuleByName(e)) || undefined === s ? undefined : s.getTaskByName(o)) && this.declareDepends(o, e);
    });
  }
  initCommandBuilder() {
    const e = this.moduleModel.getJsonObjByTargetName(this.targetName).app.bundleName;
    if (this.restoolConfigBuilder.addJsonFile(this.processedJson).addModulePackName(e).addOutputDir(this.outputDir).addResTable(path_1.resolve(this.getTaskTempDir(this.targetData, build_directory_const_js_1.BuildDirConst.R), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_H)), !this.sdkInfo.isOhos) {
      const e = path_1.resolve(this.sdkInfo.getHosToolchainsDir(), "id_defined.json");
      fs_1.existsSync(e) && this.restoolConfigBuilder.addDefinedSysIds(e);
    }
    this.targetData.getTargetName() === ohosTestTargetName && this.addOhosTestExtraCompilationResources(this.restoolConfigBuilder);
  }
  doTaskAction() {
    if (this.moduleModel.getModuleType() === module_type_enum_js_1.ModuleType.Feature) {
      const e = this.getEntryCompileResouceDir(this.targetData, this.entryModuleName);
      e && fs_extra_1.existsSync(e) && this.restoolConfigBuilder.addEntryCompiledResource(e);
    }
    this.isEnabledIconCheck() && this.restoolConfigBuilder.setIconCheck(true), this.restoolConfigBuilder.setResCompileThread(this.processResCompileThreads()), super.doTaskAction();
  }
  getEntryCompileResouceDir(e, s) {
    const t = e.findTargetDataByName(s);
    return null == t ? undefined : t.getPathInfo().getIntermediatesRes();
  }
}
