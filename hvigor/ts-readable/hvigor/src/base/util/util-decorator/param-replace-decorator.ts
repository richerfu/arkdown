/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/util-decorator/param-replace-decorator.js
 */
import * as param_replace_util_js_1 from "../param-replace-util";
import * as global_core_parameters_js_1 from "../../internal/data/global-core-parameters";
import * as common_const_js_1 from "../../common/options/common-const";
export function WithParamReplacement() {
  return function (e, r, a) {
    const t = a.value;
    return a.value = function (...e) {
      var r;
      const a = t.apply(this, e);
      if (!(e.length > 0 && "string" == typeof e[0] && e[0].endsWith(common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE))) return a;
      const o = null === (r = null === global_core_parameters_js_1.coreParameter || undefined === global_core_parameters_js_1.coreParameter ? undefined : global_core_parameters_js_1.coreParameter.startParams) || undefined === r ? undefined : r.customizedParam;
      return a instanceof Promise ? a.then(e => param_replace_util_js_1.ReplacePlaceHolderUtil.getInstance().replacePlaceholders(e, o)) : param_replace_util_js_1.ReplacePlaceHolderUtil.getInstance().replacePlaceholders(a, o);
    }, a;
  };
}
