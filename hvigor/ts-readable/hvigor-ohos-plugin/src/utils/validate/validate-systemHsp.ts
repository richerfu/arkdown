/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/validate-systemHsp.js
 */
import * as wdk_1 from "@baize/wdk";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as ohos_logger_js_1 from "../log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-validateSystemHsp");
export function validatePackageBySystemHspModel(e, o, r, t) {
  var a, s, d;
  const g = (0, wdk_1.cloneDeep)(null === (a = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === a ? undefined : a.appOpt),
    l = (0, find_target_product_js_1.findTargetProduct)(e.getProjectModel()),
    i = null !== (d = null !== (s = null == g ? undefined : g.bundleType) && undefined !== s ? s : l.bundleType) && undefined !== d ? d : o.app.bundleType;
  "shared" !== i && "appPlugin" !== i || _log.printErrorExit("BUNDLETYPE_NOT_MATCH", [i, i, t.module.name, r], [[t.module.name, i]]);
}
