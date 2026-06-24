/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-resource-increment.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as abstract_process_resource_increment_js_1 from "./abstract/abstract-process-resource-increment";
import * as task_names_js_1 from "./common/task-names";
var Task = task_names_js_1.TaskNames.Task;
export class ProcessResourceIncrement extends abstract_process_resource_increment_js_1.AbstractProcessResourceIncrement {
  constructor(e) {
    super(e, Task.PREVIEW_PROCESS_RESOURCE), this.FIRST_SUPPORT_RESTOOL_IDSMAP_APIVERSION = 10, this.restCommandContent = this.pathInfo.getIntermediatesRes(), this.outputDir = this.pathInfo.getIntermediatesRes(), this.idsMapDir = path_1.resolve(this.outputDir, build_directory_const_js_1.BuildDirConst.IDS_MAP);
  }
  initCommandBuilder() {
    this.restoolCommandBuilder = this.getRestoolCommandBuilder(), (this.service.getModuleModel().isHapModule() || this.service.getModuleModel().isHspModule()) && this.restoolCommandBuilder.addAppScopeResourcesDir(this.projectModel.getAppRes().getResourcePath());
    const e = this.projectModel.getAppRes().getAppResOpt().app.bundleName;
    this.restoolCommandBuilder.addJsonFile(this.processedJson).addModulePackName(e).addOutputDir(this.outputDir).addResTable(path_1.resolve(this.pathInfo.getGenerateSourceR(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_H)).incrementalCompile(), this.compileApiVersion >= this.FIRST_SUPPORT_RESTOOL_IDSMAP_APIVERSION && (this.restoolCommandBuilder.addIdsPath(this.idsMapDir), fs_extra_1.existsSync(path_1.resolve(this.idsMapDir, "id_defined.json")) && this.restoolCommandBuilder.addDefinedIds(this.idsMapDir)), this.sdkInfo.isOhos || this.restoolConfigBuilder.addDefinedSysIds(this.sdkInfo.getHosToolchainsDir());
  }
}
