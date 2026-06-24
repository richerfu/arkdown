/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/abstract-cache-handler.js
 */
import path_1 from "path";
export class AbstractCacheHandler {
  async onBuildEnd(e, t, a, r) {
    await this.next?.onBuildEnd(e, t, a, r);
  }
  deleteModuleCache(e, t) {
    this.next?.deleteModuleCache(e, t);
  }
  constructor(e) {
    this.cacheDir = e;
  }
  getCacheAbsolutePath() {
    return path_1.resolve(this.cacheDir, this.handlerName);
  }
  setNext(e) {
    this.next = e;
  }
  filterEvent(e, t) {
    const a = t?.getUseTime();
    e && t && a && a < 1e7 && e.deleteChild(t.getId());
  }
}
