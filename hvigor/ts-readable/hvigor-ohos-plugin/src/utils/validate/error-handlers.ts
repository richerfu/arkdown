/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/error-handlers.js
 */
import * as hvigor_1 from "@ohos/hvigor";
const _log = hvigor_1.HvigorLogger.getLogger("ValidateErrorHandlers");
export function removeLegacyPhoneSupport(e, r) {
  var o, t;
  let n = false;
  const d = null === (o = r.module) || undefined === o ? undefined : o.deviceTypes,
    s = null === (t = r.module) || undefined === t ? undefined : t.deviceType,
    a = undefined === d ? s : d;
  return Array.isArray(a) ? (e = e.filter(e => {
    const r = e.instancePath,
      o = r.lastIndexOf("["),
      t = r.substring(o + 1),
      d = parseInt(t);
    return -1 === r.indexOf("deviceType") || a.indexOf("phone") !== d || (n = true, false);
  }), n && _log.warn("Currently, the device type 'phone' is not supported in module.json5 or config.json. You are advised to change the device type to 'default'."), e) : e;
}
export function handleStartWindowErrors(e) {
  const r = ["startWindowIcon", "startWindowBackground"];
  return e.forEach(e => {
    -1 !== e.keyword.indexOf("required") && r.includes(e.params.missingProperty) && (e.solution = "For help about startWindows, you can solve the problem with this FAQ.");
  }), e;
}
export function handleEntryModulesErrors(e) {
  return e.forEach(e => {
    "entryModules" === e.instancePath && "minItems" === e.keyword && (e.reason = "The entryModule field cannot be empty, because a feature module must have a corresponding entry module. This might happen when an entry module was removed.", e.solution = "Add a dependent entry module to this feature module.");
  }), e;
}
