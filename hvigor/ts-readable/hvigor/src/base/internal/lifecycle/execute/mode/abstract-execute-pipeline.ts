/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/abstract-execute-pipeline.js
 */
import * as common_const_js_1 from "../../../../common/options/common-const";
import * as class_identify_util_js_1 from "../../../../util/class-identify-util";
import * as task_executor_js_1 from "../../../core/task-executor";
import * as task_runner_js_1 from "../../../task/core/task-runner";
import * as task_path_util_js_1 from "../../../task/util/task-path-util";
export class AbstractExecutePipeline {
  constructor(s, t) {
    this.shouldCheckTask = false, this.project = s, this.taskOptions = t;
    const e = this.taskOptions.toRunTasks;
    this.toRunTask = e.length ? e : ["default"];
  }
  async startPipeline() {
    this.taskOptions.isNeedSync || this.recordTraceData(), await this.executeTask();
  }
  findAllNeedExecuteTasksForNodes(s) {
    const t = [];
    return this.toRunTask.forEach(e => {
      const i = s.filter(s => s.hasTask(e)).map(s => (0, task_path_util_js_1.union)((0, class_identify_util_js_1.instanceOf)(s, common_const_js_1.HvigorBuildConst.PROJECT_MODE) ? "" : s.getName(), e));
      t.push(...i);
    }), t;
  }
  recordTraceData() {}
  async executeTask() {
    const s = this.taskOptions.isNeedSync ? this.findPipeAllNeedExecuteSyncTasks() : [];
    s.push(...this.findPipeAllNeedExecuteTasks()), await task_executor_js_1.taskExecutorInst.initTasks(s), await (0, task_runner_js_1.runTaskFromQueue)();
  }
}
