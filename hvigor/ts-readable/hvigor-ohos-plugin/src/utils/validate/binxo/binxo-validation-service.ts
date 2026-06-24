/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/binxo/binxo-validation-service.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../../../const/common-const";
import * as cmake_util_js_1 from "../../cmake/cmake-util";
export class BinxoValidationService {
  constructor(i) {
    var o;
    this.hwAsanConfig = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonCppConst.OHOS_ENABLE_HWASAN), this.binxoConfig = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonCppConst.OHOS_ENABLE_BINXO);
    const n = cmake_util_js_1.CmakeUtil.getCMakeArguments(null === (o = i.getBuildOption().externalNativeOptions) || undefined === o ? undefined : o.arguments);
    this.hwAsanCmake = n.includes("-DOHOS_ENABLE_HWASAN=ON"), this.binxoCmake = n.includes("-DOHOS_ENABLE_BINXO=ON");
  }
  isBinxoValid() {
    const i = this.hwAsanConfig ? "true" === this.hwAsanConfig : this.hwAsanCmake,
      o = this.binxoConfig ? "true" === this.binxoConfig : this.binxoCmake;
    return i && o;
  }
}
