/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/strategy/initialization-strategy.js
 */
export class InitializationStrategy {
  createModuleModel(e, t, i) {
    const o = e.getModuleModelByName(t.getName());
    return o.moduleBuildProfileCheck(i), o.processTargetsConfig(), o;
  }
}
