/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/validate/dependency-module-version-validate.js
 */
import * as ohos_logger_1 from "../utils/log/ohos-logger";
import * as common_const_1 from "../const/common-const";
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as file_util_1 from "../utils/file-util";
import * as config_file_loader_1 from "../utils/loader/config-file-loader";
import * as dependency_interface_1 from "../project/dependency/core/dependency-interface";
import * as json_util_1 from "../utils/json-util";
import * as common_util_1 from "../common/common-util";
const log = ohos_logger_1.OhosLogger.getLogger("DependencyModuleVersionValidate");
export function resolveJsonObjWithoutAtModule(e, o) {
  const t = [dependency_interface_1.DependencyEnum.DEPENDENCIES, dependency_interface_1.DependencyEnum.DYNAMIC_DEPENDENCIES, dependency_interface_1.DependencyEnum.DEV_DEPENDENCIES].map(e => e.toString());
  for (const n in e) if (t.includes(n)) for (const t in e[n]) e[n][t] = resolveModuleVersion(e[n][t], o);
  log.debug(`jsonObjWithoutAtModule ${JSON.stringify(e)}`);
}
export function resolveModuleVersion(e, o) {
  var t;
  const n = e.match(common_const_1.ModuleConst.REGEX);
  if (!n) return e;
  const r = n[1];
  let i = "";
  const l = null !== (t = config_file_loader_1.configFileLoader.getConfigFileJson(path_1.resolve(o, common_const_1.CommonConst.PROFILE_JSON5))) && undefined !== t ? t : {};
  if (!l.modules) return e;
  for (const e of l.modules) r === e.name && (i = file_util_1.FileUtil.convertToAbsolutePath(e.srcPath, o));
  if ("" !== i) {
    const e = path_1.resolve(i, common_const_1.CommonConst.OH_PACKAGE_JSON5),
      t = config_file_loader_1.configFileLoader.getConfigFileJson(e);
    if (t) return getParameterVersion(t.version, e, o);
    {
      const t = json_util_1.JsonUtil.getJson5Obj(e);
      return t || log.printErrorExit("MODULE_DEPENDENCY_KEY_ERROR", [r, path_1.resolve(o, common_const_1.CommonConst.OH_PACKAGE_JSON5)]), config_file_loader_1.configFileLoader.setConfigFileJson(e, t), getParameterVersion(t.version, e, o);
    }
  }
  return log.printErrorExit("MODULE_DEPENDENCY_KEY_ERROR", [r, path_1.resolve(o, common_const_1.CommonConst.OH_PACKAGE_JSON5)]), e;
}
function getParameterVersion(e, o, t) {
  const n = hvigor_1.Json5Reader.getJson5Obj(path_1.resolve(t, common_const_1.CommonConst.OH_PACKAGE_JSON5)),
    r = (0, common_util_1.replaceStringParam)(null == n ? undefined : n.parameterFile, n.properties);
  if (r) {
    const t = {
        parameterFilePath: r,
        ohPackageFilePath: o
      },
      n = hvigor_1.Json5Reader.getJson5Obj(r);
    return file_util_1.FileUtil.extracted(e, n, t);
  }
  return e;
}
