/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/lazy-task-container.js
 */
export class LazyTaskContainer {
  constructor(s) {
    this._tasks = new Map(), this._lazyTasks = new Map(), this._moduleName = s;
  }
  registerTask(s) {
    if (this._lazyTasks.has(s.details.name)) throw new Error(`Duplicate task ${s.details.name}.`);
    this._lazyTasks.set(s.details.name, s);
  }
  addTask(s) {
    this._tasks.set(s.getName(), s);
  }
  deleteTask(s) {
    return !this.hasTask(s) || this._tasks.delete(s);
  }
  hasTask(s) {
    return this._tasks.has(s) || this._lazyTasks.has(s);
  }
  getLazyTask(s) {
    return this._lazyTasks.get(s);
  }
  getTask(s) {
    return this.findTask(s);
  }
  getAllTasks() {
    return [...this._tasks.values()];
  }
  getAllTasksCA() {
    return [...this._lazyTasks.values()];
  }
  getTaskPaths() {
    return [...new Set([...this._tasks.keys(), ...this._lazyTasks.keys()])];
  }
  findTask(s) {
    const t = this._lazyTasks.get(s);
    return t ? this._tasks.has(s) ? this.initializeLazyTask(t, this._tasks.get(s)) : this.initializeLazyTask(t) : this._tasks.get(s);
  }
  findTaskCA(s) {
    return this._lazyTasks.get(s);
  }
  getTaskDepends(s) {
    const t = this._tasks.get(s);
    if (t) return this.mergeDepends(this._lazyTasks.get(s), t);
    const e = this._lazyTasks.get(s);
    if (!e) throw new Error(`Task ${s} not find.`);
    return e.depends;
  }
  clearTasks() {
    this._lazyTasks.clear(), this._tasks.clear();
  }
  initializeLazyTask(s, t) {
    const e = null != t ? t : s.provider();
    return this._tasks.set(s.details.name, e), e.setDependsOn(...this.mergeDepends(s, e));
  }
  mergeDepends(s, t) {
    var e, a;
    return [...new Set([...(null !== (e = null == t ? undefined : t.getDependsOn()) && undefined !== e ? e : []), ...(null !== (a = null == s ? undefined : s.depends) && undefined !== a ? a : [])])];
  }
}
