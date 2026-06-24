/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-control-center.js
 */
import events_1 from "events";
import fs_1 from "fs";
import * as ts_check_js_1 from "../../util/ts_check";
import * as task_module_util_js_1 from "../util/task-module-util";
import * as task_path_util_js_1 from "../util/task-path-util";
import * as task_directed_acyclic_graph_js_1 from "./task-directed-acyclic-graph";
import * as task_queue_js_1 from "./task-queue";
import * as parse_json_file_js_1 from "../../../../common/util/parse-json-file";
import * as hvigor_core_js_1 from "../../../external/core/hvigor-core";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
export class TaskControlCenter extends events_1 {
  constructor() {
    super(), this._log = hvigor_log_js_1.HvigorLogger.getLogger("taskControlCenter"), this.blockedTaskMap = new Map(), this.runnableTaskQueue = new task_queue_js_1.TaskQueue(), this.runningTaskList = new Map();
  }
  clear() {
    this.clearRunnableTaskQueue(), this.clearBlockedTaskMap(), this.clearInterval(), this.runningTaskList.clear();
  }
  initTaskQueueBeforeRun(e) {
    e.forEach(e => this.initTaskQueueForOneTaskNew(e));
  }
  initTaskQueueForOneTaskNew(e) {
    this.updateTargetRoots(e);
    const t = [...task_directed_acyclic_graph_js_1.projectTaskDag.getChildren(e).values()];
    this.blockedTaskMap.set(e, t.length), 0 === t.length ? this.runnableTaskQueue.push(e) : t.forEach(e => {
      this.blockedTaskMap.has(e) || this.initTaskQueueForOneTaskNew(e);
    });
  }
  updateTargetRoots(e) {
    const {
        moduleName: t
      } = (0, task_path_util_js_1.parseTaskPath)(e),
      s = ts_check_js_1.targetModulesRoots.get(t);
    if (ts_check_js_1.targetModules.has(t) && s && fs_1.existsSync(s)) return;
    const r = (0, task_module_util_js_1.getModuleFromTaskPath)(e);
    if (!r) throw new Error(`Can not find node ${t} with task ${e}`);
    ts_check_js_1.hvigorConfigFileTsRoot && ts_check_js_1.targetRoots.add(ts_check_js_1.hvigorConfigFileTsRoot);
    const a = `${r.getBuildFilePath()}.ts`;
    if (!ts_check_js_1.hvigorfileTSRoots.has(a)) return;
    ts_check_js_1.targetRoots.add(a), ts_check_js_1.targetModules.add(t), ts_check_js_1.targetModulesRoots.set(t, a);
    const o = parse_json_file_js_1.ParseJsonFile.parseJsonFile(r.getBuildProfilePath()).entryModules;
    if (!o) return;
    const i = r.getProject();
    for (const e of o) {
      const s = i.findModuleByName(e);
      if (!s) throw new Error(`Invalid related node ${e} from node ${t} `);
      const r = `${s.getBuildFilePath()}.ts`;
      ts_check_js_1.hvigorfileTSRoots.has(r) && (ts_check_js_1.targetRoots.add(r), ts_check_js_1.targetModules.add(e), ts_check_js_1.targetModulesRoots.set(e, r));
    }
  }
  clearRunnableTaskQueue() {
    this.runnableTaskQueue.clear();
  }
  clearBlockedTaskMap() {
    this.blockedTaskMap.clear();
  }
  allTasksHasExecuted() {
    return 0 === this.blockedTaskMap.size;
  }
  popRunnableTask() {
    return this.runnableTaskQueue.pop();
  }
  getRunnableTaskSize() {
    return this.runnableTaskQueue.size();
  }
  setTaskExecuted(e) {
    this.blockedTaskMap.delete(e);
    task_directed_acyclic_graph_js_1.projectTaskDag.getParent(e).forEach(e => {
      if (!this.blockedTaskMap.has(e)) return;
      const t = this.blockedTaskMap.get(e) - 1;
      this.blockedTaskMap.set(e, t), 0 === t && this.runnableTaskQueue.push(e);
    });
  }
  pushRunningTask(e) {
    this.runningTaskList.set(e.getPath(), e);
  }
  getRunningTask(e) {
    return this.runningTaskList.get(e);
  }
  setIntervalId(e) {
    this.intervalId = e;
  }
  clearInterval() {
    clearInterval(this.intervalId);
  }
  popPriorTask() {
    if (0 === hvigor_core_js_1.hvigorCore.getCompileDependArr().size) return;
    const e = this.runnableTaskQueue.popPriorTask(hvigor_core_js_1.hvigorCore.getCompileDependArr());
    return e && this._log.debug("Poping task with high priority: " + e), e;
  }
}
export const taskControlCenter = new TaskControlCenter();
