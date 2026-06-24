/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/abstract-build-profile-loader.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../../../const/common-const";
import * as json_util_js_1 from "../../json-util";
import * as ohos_logger_js_1 from "../../log/ohos-logger";
import * as validate_util_js_1 from "../../validate/validate-util";
import * as validator_store_js_1 from "../../validate/validator-store";
import * as config_file_loader_js_1 from "../config-file-loader";
export class AbstractBuildProfileLoader {
  constructor() {
    this._log = ohos_logger_js_1.OhosLogger.getLogger("BuildProfileLoader");
  }
  getBuildProfileJson5Obj(e, o = true, t) {
    const i = hvigor_1.hvigor.getNodeByNameInternal(e, t);
    if (undefined === i) return void this._log.printErrorExit("NODE_NOT_FOUND_BY_NODE_NAME", [e]);
    const r = path_1.resolve(i.getNodePathInternal(), common_const_js_1.CommonConst.PROFILE_JSON5),
      s = config_file_loader_js_1.configFileLoader.getConfigFileJson(r);
    if (undefined !== s) return s;
    if (fs_extra_1.existsSync(r)) {
      let t = json_util_js_1.JsonUtil.getJson5Obj(r);
      return t = this.initializeReplace(t), this.setBuildProfileJson5Obj(e, t, o), t;
    }
    this._log.warn(`not found build-profile json file in : ${r}`);
  }
  setBuildProfileJson5ObjCheck(e, o, t, i) {
    const r = hvigor_1.hvigor.getNodeByNameInternal(e, i);
    if (undefined === r) return void this._log.printErrorExit("NODE_NOT_FOUND_BY_NODE_NAME", [e]);
    const s = path_1.resolve(r.getNodePathInternal(), common_const_js_1.CommonConst.PROFILE_JSON5);
    config_file_loader_js_1.configFileLoader.setConfigFileJson(s, o), t && this.validateBuildProfile(o, s, t);
  }
  validateBuildProfile(e, o, t) {
    const i = validator_store_js_1.ValidatorStore.addValidator(t);
    try {
      validate_util_js_1.ValidateUtil.schemaCheckJSObject(e, undefined, i);
    } catch (e) {
      e instanceof Error && this._log.errorMessageExit(e.name, e.message);
    }
  }
  initializeReplace(e) {
    return e;
  }
}
