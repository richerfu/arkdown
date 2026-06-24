/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/global/project-ohos-config-manager.js
 */
export class ProjectOhosConfigManager {
  loaderConfig(o) {
    this.ohosConfig = o;
  }
  getOverrides() {
    var o;
    return null === (o = this.ohosConfig) || undefined === o ? undefined : o.overrides;
  }
  getConfig() {
    return this.ohosConfig;
  }
}
export const projectOhosConfigManager = new ProjectOhosConfigManager();
