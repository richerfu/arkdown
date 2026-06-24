/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/data/global-store.js
 */
class GlobalDataStore {
  constructor() {
    this.store = new Map();
  }
  set(t, e) {
    return this.store.set(t, e), this;
  }
  get(t) {
    return this.store.get(t);
  }
  static getInstance() {
    return GlobalDataStore.instance || (GlobalDataStore.instance = new GlobalDataStore()), GlobalDataStore.instance;
  }
}
export default GlobalDataStore.getInstance();
