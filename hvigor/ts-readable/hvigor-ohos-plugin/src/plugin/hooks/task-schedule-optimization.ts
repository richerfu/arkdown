/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/task-schedule-optimization.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as ohos_logger_1 from "../../utils/log/ohos-logger";
export class TaskScheduleOptimizationService {
  constructor(e) {
    this._log = ohos_logger_1.OhosLogger.getLogger(TaskScheduleOptimizationService.name), this.project = e, this.visitedTasks = new Set();
  }
  checkSupportedEntryTasks() {
    var e;
    const s = null === (e = null === hvigor_1.globalData || undefined === hvigor_1.globalData ? undefined : hvigor_1.globalData.cliOpts) || undefined === e ? undefined : e._;
    for (const e of s) if (!TaskScheduleOptimizationService.SUPPORTED_ASSEMBLE_TASKS.includes(e)) return this._log.debug("Added entry task that is not supported for schedule optimization: ", e), false;
    return true;
  }
  computeAndSetCompileDependencies() {
    if (this.checkSupportedEntryTasks()) {
      for (const e of this.project.getSubModules().values()) {
        const s = this.extractCompileTasks(e);
        0 !== s.length && this.processTaskQueue([...s]);
      }
      this.visitedTasks.add(":clean"), this._log.debug("Prior task list for the project : " + this.visitedTasks.toString()), hvigor_1.hvigorCore.setCompileDependArr(this.visitedTasks);
    } else hvigor_1.hvigorCore.setCompileDependArr(new Set());
  }
  extractCompileTasks(e) {
    return e.getTaskPaths().filter(e => e.includes("CompileArkTS"));
  }
  processTaskQueue(e) {
    for (; e.length > 0;) {
      const s = e.shift();
      this.visitedTasks.has(s) || (this.visitedTasks.add(s), this.addTaskDependenciesToQueue(e, s));
    }
  }
  addTaskDependenciesToQueue(e, s) {
    const i = this.getSafeTaskDependencies(s);
    null == i || i.forEach(s => {
      this.visitedTasks.has(s) || e.push(s);
    });
  }
  getSafeTaskDependencies(e) {
    return hvigor_1.projectTaskDag.getChildren(e);
  }
}
TaskScheduleOptimizationService.SUPPORTED_ASSEMBLE_TASKS = ["assembleHap", "assembleHsp", "assembleHar", "assembleApp"];
