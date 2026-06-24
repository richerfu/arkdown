/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/task/incremental-task.js
 */
import * as file_set_js_1 from "../../internal/snapshot/util/file-set";
import * as default_task_js_1 from "./default-task";
export class IncrementalTask extends default_task_js_1.DefaultTask {
  declareInputs() {
    return new Map();
  }
  declareInputFiles() {
    return new file_set_js_1.FileSet();
  }
  declareOutputFiles() {
    return new file_set_js_1.FileSet();
  }
  async beforeAlwaysAction() {}
}
