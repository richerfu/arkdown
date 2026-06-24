/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/module-info/ohos-uitransform-optimization.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../../const/common-const";
export const getOhosUiTransformOptimization = o => !!o.isByteCodeHar() || true === hvigor_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.CommonConst.OHOS_UI_TRANSFORM_OPTIMIZATION);
