/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/custom-hook-service.js
 */
const ALL_TARGET = "all";
class CustomHookActions {
  constructor() {
    this.customTypeActions = new Map();
  }
  registerAction(o, t, s) {
    const e = this.customTypeActions.get(o) || new Map();
    e.set(null != s ? s : "all", t), this.customTypeActions.set(o, e);
  }
  getAction(o, t) {
    var s;
    const e = this.customTypeActions.get(o);
    if (e) return null !== (s = e.get(t)) && undefined !== s ? s : e.get("all");
  }
  clean() {
    this.customTypeActions.clear();
  }
}
export class CustomHookActionService {
  constructor() {
    this.moduleCustomHookActions = new Map();
  }
  registerCustomAction(o, t, s, e) {
    var c;
    const i = null !== (c = this.moduleCustomHookActions.get(o)) && undefined !== c ? c : new CustomHookActions();
    i.registerAction(t, s, e), this.moduleCustomHookActions.set(o, i);
  }
  getCustomAction(o, t, s) {
    const e = this.moduleCustomHookActions.get(o);
    if (e) return e.getAction(t, s);
  }
  clean() {
    this.moduleCustomHookActions.clear();
  }
}
export const customHookActionService = new CustomHookActionService();
