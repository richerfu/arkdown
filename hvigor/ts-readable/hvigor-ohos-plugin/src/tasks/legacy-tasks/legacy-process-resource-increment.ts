/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-process-resource-increment.js
 */
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as abstract_process_resource_increment_js_1 from "../abstract/abstract-process-resource-increment";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_hook_compile_resource_js_1 from "./hook/legacy-hook-compile-resource";
import path_1 from "path";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyProcessResourceIncrement extends abstract_process_resource_increment_js_1.AbstractProcessResourceIncrement {
  constructor(e, s) {
    var t;
    super(e, (0, legacy_hook_compile_resource_js_1.getTaskRealName)(LegacyFATask.PREVIEW_PROCESS_RESOURCE, s)), this.entryModuleName = s, this.outputDir = path_1.resolve(this.pathInfo.getIntermediatesRes(), null !== (t = this.entryModuleName) && undefined !== t ? t : "");
  }
  initCommandBuilder() {
    const e = this.moduleModel.getJsonObjByTargetName(this.targetName).app.bundleName;
    this.restoolCommandBuilder = this.getRestoolCommandBuilder(), this.restoolCommandBuilder.addJsonFile(this.processedJson).addModulePackName(e).addOutputDir(this.outputDir).addResTable(path_1.resolve(this.getTaskTempDir(this.targetData, build_directory_const_js_1.BuildDirConst.R), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_H)).incrementalCompile(), this.sdkInfo.isOhos || this.restoolConfigBuilder.addDefinedSysIds(this.sdkInfo.getHosToolchainsDir());
  }
}
