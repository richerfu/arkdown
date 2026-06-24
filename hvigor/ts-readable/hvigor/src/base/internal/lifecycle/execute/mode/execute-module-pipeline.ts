/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/execute-module-pipeline.js
 */
import path_1 from "path";
import * as index_js_1 from "../../../../../../index";
import * as node_config_path_info_js_1 from "../../../../../common/node-config-path-info";
import * as hvigor_log_js_1 from "../../../../log/hvigor-log";
import * as sync_js_1 from "../../sync";
import * as abstract_execute_pipeline_js_1 from "./abstract-execute-pipeline";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-module-pipeline");
export class ExecuteModulePipeline extends abstract_execute_pipeline_js_1.AbstractExecutePipeline {
  constructor(e, t) {
    super(e, t), this.executeModules = [], this.init();
  }
  init() {
    const e = index_js_1.hvigorCore.getExtraConfig().get("module");
    undefined === e ? this.executeModules = this.project.getAllSubModules() : (this.shouldCheckTask = true, e.split(",").forEach(e => {
      const t = e.split("@")[0],
        i = this.project.findModuleByName(t);
      undefined === i && (node_config_path_info_js_1.nodeConfigPathInfo.checkExcludeNode(t) && _log.printErrorExit("UNKNOWN_MODULE_IN_COMMAND_LINE_HVIGORCONFIG", [t, path_1.resolve(this.project.getNodeDir(), "hvigorconfig.ts")]), _log.printErrorExit("UNKNOWN_MODULE_IN_COMMAND_LINE_BUILD_PROFILE", [t, this.project.getBuildProfilePath()])), this.executeModules.push(i);
    }));
  }
  findPipeAllNeedExecuteSyncTasks() {
    const e = [];
    return this.executeModules.forEach(t => {
      e.push(...(0, sync_js_1.findSyncTaskPathsForNode)(t));
    }), e;
  }
  findPipeAllNeedExecuteTasks() {
    return this.findAllNeedExecuteTasksForNodes([...this.executeModules]);
  }
  recordTraceData() {}
}
