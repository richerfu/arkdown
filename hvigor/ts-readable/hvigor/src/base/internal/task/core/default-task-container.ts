/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/default-task-container.js
 */
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as task_directed_acyclic_graph_js_1 from "./task-directed-acyclic-graph";
export class DefaultTaskContainer {
  constructor(t, s) {
    this._log = hvigor_log_js_1.HvigorLogger.getLogger(DefaultTaskContainer.name), this._moduleName = t, this._tasks = new Map(), this._taskGraph = s;
  }
  registryTask(t) {
    throw new Error("Method not implemented.");
  }
  getTaskPaths() {
    return [...this._tasks.keys()];
  }
  registerTask(t) {
    throw new Error("Method not implemented.");
  }
  getTaskDepends(t) {
    const s = this.findTask(t);
    if (!s) throw new Error(`Task not found ${t}.`);
    return s.getDependsOn();
  }
  addTask(t) {
    const s = t.getName();
    this._tasks.has(s) && this._log.printErrorExit("DUPLICATE_TASK", [s, t.getPath()]), this._tasks.set(s, t), this._taskGraph.addNode(t.getPath()), task_directed_acyclic_graph_js_1.projectTaskDag.addNode(t.getPath());
  }
  deleteTask(t) {
    if (this.hasTask(t)) {
      const s = this._tasks.get(t).getPath();
      return this._taskGraph.removeNode(s), task_directed_acyclic_graph_js_1.projectTaskDag.removeNode(s), this._tasks.delete(t), true;
    }
    return false;
  }
  findTask(t) {
    return this._tasks.get(t);
  }
  getAllTasks() {
    return [...this._tasks.values()];
  }
  hasTask(t) {
    return this._tasks.has(t);
  }
  clearTasks() {
    this._tasks.clear(), this._taskGraph.clear();
  }
}
