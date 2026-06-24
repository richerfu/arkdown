/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/oh-modules-lock-loader.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as const_js_1 from "@ohos/hvigor/src/common/const/const";
import * as path_const_1 from "@ohos/hvigor/src/common/const/path-const";
import * as common_const_js_1 from "../../../const/common-const";
import * as json_util_js_1 from "../../json-util";
import * as config_file_loader_js_1 from "../config-file-loader";
class OhModulesLockLoader {
  constructor() {
    this.lockFilePath = path_1.resolve(path_const_1.HVIGOR_PROJECT_ROOT_DIR, common_const_js_1.CommonConst.OH_MODULES, common_const_js_1.CommonConst.OHPM, common_const_js_1.CommonConst.LOCK_JSON5);
  }
  getLockJsonObj() {
    let e = config_file_loader_js_1.configFileLoader.getConfigFileJson(this.lockFilePath);
    return undefined === e && (config_file_loader_js_1.configFileLoader.setConfigFileJson(this.lockFilePath, json_util_js_1.JsonUtil.getJson5Obj(this.lockFilePath)), e = config_file_loader_js_1.configFileLoader.getConfigFileJson(this.lockFilePath)), null != e ? e : {};
  }
  isModuleInProjectPkgJsonOverrideDependencies(e) {
    var o, n;
    const i = path_1.resolve(path_const_1.HVIGOR_PROJECT_ROOT_DIR, common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
    return !!(null === (n = (null !== (o = config_file_loader_js_1.configFileLoader.getConfigFileJson(i)) && undefined !== o ? o : {}).overrideDependencyMap) || undefined === n ? undefined : n[e]);
  }
  isNeedOverrideWithLockJson(e) {
    return true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(const_js_1.ENABLE_OVERRIDES_DEPENDENCY_MAP) && this.isModuleInProjectPkgJsonOverrideDependencies(e);
  }
  getOverrideDependencyPkgObj(e, o) {
    var n, i, s, t;
    const r = (0, wdk_1.cloneDeep)(o),
      _ = null === (n = this.getLockJsonObj().overrideDependencyMap) || undefined === n ? undefined : n[e];
    return r && _ && (r.dependencies = null !== (i = _.dependencies) && undefined !== i ? i : {}, r.devDependencies = null !== (s = _.devDependencies) && undefined !== s ? s : {}, r.dynamicDependencies = null !== (t = _.dynamicDependencies) && undefined !== t ? t : {}), r;
  }
}
export const ohModulesLockLoader = new OhModulesLockLoader();
