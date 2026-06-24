/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/execute-project-pipeline.js
 */
import * as sync_js_1 from "../../sync";
import * as abstract_execute_pipeline_js_1 from "./abstract-execute-pipeline";
export class ExecuteProjectPipeline extends abstract_execute_pipeline_js_1.AbstractExecutePipeline {
  constructor(e, t, s) {
    super(e, t), this.shouldCheckTask = s;
  }
  findPipeAllNeedExecuteSyncTasks() {
    return (0, sync_js_1.findSyncTaskPathsForNode)(this.project);
  }
  findPipeAllNeedExecuteTasks() {
    return this.findAllNeedExecuteTasksForNodes([this.project]);
  }
}
