/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/const/version-const.js
 */
import * as hos_sdkmanager_common_1 from "@ohos/hos-sdkmanager-common";
export class VersionConst {}
function getHosCompileVersion() {
  const e = getLatestSupportVersion();
  return `${e.getPlatformVersion().getVersion()}(${e.getFullBaseApi().getValue()})`;
}
function getCurrentModelVersion() {
  return getLatestSupportVersion().getPlatformVersion().getVersion();
}
function getLatestSupportVersion() {
  return hos_sdkmanager_common_1.HosVersionMapper.INSTANCE.getLatestSupportVersion();
}
VersionConst.SUPPORT_COMPILE_VERSION = getHosCompileVersion(), VersionConst.MINIMUM_COMPATIBLE_VERSION = "4.0.0(10)", VersionConst.ATOMIC_SERVICE_MINIMUM_COMPATIBLE_VERSION = "4.1.0(11)", VersionConst.CURRENT_MODEL_VERSION = getCurrentModelVersion(), VersionConst.MINIMUM_MODEL_VERSION = "5.0.0";
