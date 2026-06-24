/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/execute-undefined-pipeline.js
 */
import * as abstract_execute_pipeline_js_1 from "./abstract-execute-pipeline";
import * as execute_module_pipeline_js_1 from "./execute-module-pipeline";
import * as execute_project_pipeline_js_1 from "./execute-project-pipeline";
export class ExecuteUndefinedPipeline extends abstract_execute_pipeline_js_1.AbstractExecutePipeline {
  constructor(e, i) {
    super(e, i), this.projectPipeLine = new execute_project_pipeline_js_1.ExecuteProjectPipeline(e, i, false), this.modulePipeline = new execute_module_pipeline_js_1.ExecuteModulePipeline(e, i);
  }
  findPipeAllNeedExecuteSyncTasks() {
    return [...this.modulePipeline.findPipeAllNeedExecuteSyncTasks(), ...this.projectPipeLine.findPipeAllNeedExecuteSyncTasks()];
  }
  findPipeAllNeedExecuteTasks() {
    return [...this.modulePipeline.findPipeAllNeedExecuteTasks(), ...this.projectPipeLine.findPipeAllNeedExecuteTasks()];
  }
}
