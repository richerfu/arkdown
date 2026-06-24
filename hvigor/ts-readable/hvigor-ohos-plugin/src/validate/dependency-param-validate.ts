/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/validate/dependency-param-validate.js
 */
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as semver_1 from "semver";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as npm_utils_js_1 from "../utils/npm-utils";
import * as dependency_tag_validate_js_1 from "./dependency-tag-validate";
const log = ohos_logger_js_1.OhosLogger.getLogger("ParameterFileLog"),
  PARAMETER_KEY_REG = /(?=^(@(?![0-9\-_])[a-z0-9\-_]+(?<![-_])\/)?(?![0-9\-_.])[a-z0-9\-_.]+(?<![-_.])$).{1,128}$/i;
function matchValueWithLongestPrefixStrategy(e, t) {
  let a = "";
  for (let r = e.length; r > 0; r--) {
    if (a = getValueFromJson(e.slice(0, r).join("."), a, t), "string" == typeof a) return a;
    if (a && "object" == typeof a && r !== e.length && (a = matchValueWithLongestPrefixStrategy(e.slice(r, e.length), a), a)) return a;
  }
  return "";
}
export function matchValueWithParameterizationKey({
  keys: e,
  parameterFileJson: t,
  pathParamsObj: a,
  ohPackageInfoGetter: r,
  dependencyName: o
}) {
  const n = matchValueWithLongestPrefixStrategy(e, t);
  return !n && a.parameterFilePath && log.printErrorExit("INVALID_PARAMETER_KEYS", [e.join(".")], [[a.parameterFilePath]]), processParameterizationValue({
    value: n,
    key: e.join("."),
    parameterFileJson: t,
    pathParamsObj: a,
    ohPackageInfoGetter: r,
    dependencyName: o
  });
}
function getValueFromJson(e, t, a) {
  return t = getObjectValueByKey(e, t || a);
}
function getObjectValueByKey(e, t) {
  return "string" == typeof t ? t : t[e];
}
export function validateParameterKeys(e) {
  Object.keys(e).forEach(t => {
    PARAMETER_KEY_REG.test(t) || log.printErrorExit("INCORRECT_PARAMETER_KEYS_FORMAT", [t]), "object" == typeof e[t] && null !== e[t] && validateParameterKeys(e[t]);
  });
}
function processParameterizationValue(e) {
  var t;
  const {
    value: a,
    key: r,
    pathParamsObj: o,
    dependencyName: n
  } = e;
  return "latest" === a || "*" === a ? {
    value: a
  } : (0, npm_utils_js_1.isLocalDependency)(a) ? e.ohPackageInfoGetter && e.dependencyName && (null === (t = e.pathParamsObj) || undefined === t ? undefined : t.parameterFilePath) ? handleParameterizationLocalDependencies(e) : {
    value: `${common_const_js_1.ParameterizationConst.PREFIX}${r}`,
    localDepArr: []
  } : ((0, dependency_tag_validate_js_1.isValidateTag)({
    key: r,
    value: a,
    parameterFilePath: o.parameterFilePath,
    ohPackageFilePath: o.ohPackageFilePath,
    dependencyName: n
  }) || (0, semver_1.validRange)(a, {
    loose: true,
    includePrerelease: true
  }) || log.printErrorExit("INVALID_PARAMETER_VALUES", [a]), {
    value: a
  });
}
function handleParameterizationLocalDependencies(e) {
  const {
      value: t,
      pathParamsObj: a,
      ohPackageInfoGetter: r,
      dependencyName: o,
      parameterFileJson: n
    } = e,
    _ = t.startsWith(common_const_js_1.CommonConst.LOCAL_DEPENDENCY_PREFIX) ? t.slice(common_const_js_1.CommonConst.LOCAL_DEPENDENCY_PREFIX.length) : t,
    i = path_1.resolve(path_1.dirname(a.parameterFilePath), _);
  let s = false;
  try {
    s = fs_extra_1.realpathSync.native(i) === i;
  } catch (e) {
    s = false;
  }
  if (s || log.printErrorExit("LOCAL_DEPENDENCY_IN_PARAMETER_FILE_DOES_NOT_EXIST", [], [[i, a.ohPackageFilePath, o]]), fs_extra_1.statSync(i).isDirectory()) {
    r(path_1.resolve(i, common_const_js_1.CommonConst.OH_PACKAGE_JSON5)) || log.printErrorExit("UNABLE_TO_GET_VERSION_AND_NAME_IN_OH_PACKAGE_JSON5", [o, path_1.resolve(i, common_const_js_1.CommonConst.OH_PACKAGE_JSON5)], [[a.parameterFilePath]]);
    const {
      version: e,
      name: t
    } = r(path_1.resolve(i, common_const_js_1.CommonConst.OH_PACKAGE_JSON5));
    o !== t && log.printErrorExit("THERE_ARE_SOME_DEPENDENCY_NAMES_THAT_ARE_INCONSISTENT_WITH_THE_ACTUAL_PACKAGE_NAMES", [], [[o, a.ohPackageFilePath, t, t]]);
    const _ = e.match(common_const_js_1.ParameterizationConst.REGEX);
    if (_) {
      const {
        value: e
      } = matchValueWithParameterizationKey({
        keys: _[1].split("."),
        parameterFileJson: n,
        pathParamsObj: a,
        dependencyName: "version"
      });
      return {
        value: e
      };
    }
    return {
      value: e
    };
  }
  const l = path_1.extname(i).toLowerCase();
  if (".har" === l || ".tgz" === l) {
    const e = path_1.dirname(a.ohPackageFilePath);
    return {
      value: (0, hvigor_common_1.isSubPath)(i, e) ? `${common_const_js_1.CommonConst.LOCAL_DEPENDENCY_PREFIX}${path_1.relative(e, i)}` : `${common_const_js_1.CommonConst.LOCAL_DEPENDENCY_PREFIX}${path_1.basename(i)}`,
      localDepArr: [i]
    };
  }
  log.printErrorExit("UNRECOGNIZED_ARCHIVE_FORMAT_IN_PARAMETER_FILE", [], [[i, a.ohPackageFilePath, o]]);
}
