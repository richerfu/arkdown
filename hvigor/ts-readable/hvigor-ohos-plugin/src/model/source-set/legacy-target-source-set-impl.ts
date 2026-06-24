/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/source-set/legacy-target-source-set-impl.js
 */
import * as legacy_target_res_model_impl_js_1 from "../res/legacy-target-res-model-impl";
import * as core_source_set_impl_js_1 from "./core-source-set-impl";
export class LegacyTargetSourceSetImpl extends core_source_set_impl_js_1.CoreSourceSetImpl {
  constructor(e) {
    super(e, new legacy_target_res_model_impl_js_1.LegacyModuleTargetResImpl(e));
  }
  getLegacyModuleTargetRes() {
    return this.targetResModel;
  }
  getTargetSourceSetModuleName() {
    var e, r, t;
    return null === (t = null === (r = null === (e = this.getLegacyModuleTargetRes()) || undefined === e ? undefined : e.getConfigJsonOpt()) || undefined === r ? undefined : r.module) || undefined === t ? undefined : t.name;
  }
}
