/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/global/module-ohos-config-manager.js
 */
export class ModuleOhosConfigManager {
  constructor() {
    this.configMap = new Map();
  }
  loaderConfig(o, e) {
    this.configMap.set(o, e);
  }
  getBuildOption(o) {
    var e, n;
    return null === (n = null === (e = this.configMap.get(o)) || undefined === e ? undefined : e.overrides) || undefined === n ? undefined : n.buildOption;
  }
  getConfig(o) {
    return this.configMap.get(o);
  }
}
export const moduleOhosConfigManager = new ModuleOhosConfigManager();
