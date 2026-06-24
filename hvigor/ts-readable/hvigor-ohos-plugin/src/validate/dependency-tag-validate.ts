/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/validate/dependency-tag-validate.js
 */
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
const TAG_REG = /^[A-Za-z0-9][A-Za-z0-9._-]{0,59}$/,
  _log = ohos_logger_js_1.OhosLogger.getLogger("TagValidate");
export function isValidateTag(e) {
  const {
    key: t,
    value: a,
    parameterFilePath: o,
    ohPackageFilePath: r,
    dependencyName: s
  } = e;
  return !!a.startsWith("tag:") && (TAG_REG.test(a.split("tag:")[1]) || _log.printErrorExit("INVALID_TAG_%S_AT_%S", [a.split("tag:")[1], o], [[a.split("tag:")[1], t, o]]), "version" === s && _log.printErrorExit("THE_TAG_KEYWORD_IS_NOT_ALLOWED_FOR_VERSION_AT_%S", [r], [[a, t, o]]), true);
}
