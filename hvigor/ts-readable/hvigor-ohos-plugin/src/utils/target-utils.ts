/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/target-utils.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../const/common-const";
import * as inject_util_js_1 from "./inject-util";
export class TargetUtils {
  static pushTryTargets(t) {
    var e;
    const s = [],
      o = null === (e = t.projectModel.getModuleSpecificTargets().get(t.moduleName)) || undefined === e ? undefined : e[0];
    return inject_util_js_1.InjectUtil.isOhosTest() || inject_util_js_1.InjectUtil.isLocalTest() ? o === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET ? (s.push(common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET), s) : (s.push(common_const_js_1.DefaultTargetConst.DEFAULT_TARGET), s) : ((0, hvigor_1.getAlignTarget)() && s.push((0, hvigor_1.getAlignTarget)()), o && o !== common_const_js_1.DefaultTargetConst.ALL_TARGET && s.push(o), s.push(t.superTargetName), s.push(...this.getFallbackTargets(), common_const_js_1.DefaultTargetConst.DEFAULT_TARGET), s);
  }
  static computeTargets(t, e) {
    const s = null != t ? t : ["all"],
      o = (0, hvigor_1.getAlignTarget)();
    if (!o || inject_util_js_1.InjectUtil.isLocalTest() || inject_util_js_1.InjectUtil.isOhosTest()) return s;
    if (e.some(t => t.name === o)) return [o];
    const r = this.getFallbackTargets();
    for (const t of r) {
      if (e.some(e => e.name === t)) return [t];
    }
    return ["default"];
  }
  static getFallbackTargets() {
    const t = hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.DefaultTargetConst.FALLBACK_TARGET);
    return t ? "string" == typeof t ? t.split(",") : t : [];
  }
}
