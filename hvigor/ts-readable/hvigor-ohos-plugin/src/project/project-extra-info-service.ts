/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/project-extra-info-service.js
 */
import * as find_target_product_js_1 from "../common/find-target-product";
import * as common_const_js_1 from "../const/common-const";
import * as runtime_type_enum_js_1 from "../enum/runtime-type-enum";
export class ProjectExtraInfoService {
  static getProjectRuntimeOS(e) {
    const t = (0, find_target_product_js_1.findTargetProduct)(e);
    return t.runtimeOS ? runtime_type_enum_js_1.RuntimeTypeEnum.valueOf(t.runtimeOS) : t.compatibleSdkVersion && !t.compatibleSdkVersion.toString().includes("(") ? runtime_type_enum_js_1.RuntimeTypeEnum.valueOf(common_const_js_1.CommonConst.OPEN_HARMONY) : runtime_type_enum_js_1.RuntimeTypeEnum.valueOf(common_const_js_1.CommonConst.HARMONY_OS);
  }
}
