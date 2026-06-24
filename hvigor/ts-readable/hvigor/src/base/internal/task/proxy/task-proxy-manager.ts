/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/proxy/task-proxy-manager.js
 */
class TaskProxyManager {
  constructor() {
    this.reset();
  }
  reset() {
    this.proxyCache = new Map();
  }
  set(e, r) {
    return this.proxyCache.set(e, r), r;
  }
  getTaskProxy(e) {
    if (!this.proxyCache.has(e)) throw new Error(`${e} is not an incremental task.`);
    return this.proxyCache.get(e);
  }
  getAllIncrementalTaskProxy() {
    return [...this.proxyCache.values()];
  }
}
export const taskProxyManager = new TaskProxyManager();
