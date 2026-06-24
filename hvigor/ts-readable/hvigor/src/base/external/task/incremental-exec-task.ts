/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/task/incremental-exec-task.js
 */
import * as incremental_task_js_1 from "./incremental-task";
export class IncrementalExecTask extends incremental_task_js_1.IncrementalTask {
  declareExecutionTool() {
    return "";
  }
  declareExecutionCommand() {
    return "";
  }
  declareExecutionEnv() {
    return new Map();
  }
}
