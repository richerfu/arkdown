/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/source-set/target-source-set-impl.js
 */
import * as target_res_model_impl_js_1 from "../res/target-res-model-impl";
import * as core_source_set_impl_js_1 from "./core-source-set-impl";
export class TargetSourceSetImpl extends core_source_set_impl_js_1.CoreSourceSetImpl {
  constructor(e) {
    super(e, new target_res_model_impl_js_1.ModuleTargetResImpl(e));
  }
  getModuleTargetRes() {
    return this.targetResModel;
  }
  getTargetSourceSetModuleName() {
    var e, r, t;
    return null === (t = null === (r = null === (e = this.getModuleTargetRes()) || undefined === e ? undefined : e.getModuleJsonOpt()) || undefined === r ? undefined : r.module) || undefined === t ? undefined : t.name;
  }
}
