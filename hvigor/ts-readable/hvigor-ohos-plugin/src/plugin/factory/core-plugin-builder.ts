/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/core-plugin-builder.js
 */
export class CorePluginBuilder {
  constructor(e) {
    this.module = e;
  }
  createModuleModel(e, o) {
    const r = e.getModuleModelByName(this.module.getName());
    return r.moduleBuildProfileCheck(o), r.processTargetsConfig(), r;
  }
}
