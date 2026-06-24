/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/res-model-loader.js
 */
import fs_extra_1 from "fs-extra";
import * as json_util_js_1 from "../../json-util";
import * as ohos_logger_js_1 from "../../log/ohos-logger";
import * as validate_util_js_1 from "../../validate/validate-util";
import * as validator_store_js_1 from "../../validate/validator-store";
import * as config_file_loader_js_1 from "../config-file-loader";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
class ResModelLoader {
  constructor() {
    this._log = ohos_logger_js_1.OhosLogger.getLogger("ResModelLoader"), this.appJsonSchemaPath = undefined, this.moduleJsonSchemaPath = undefined;
  }
  getAppJson(e) {
    return this.getResModelJsonObj(e, this.appJsonSchemaPath);
  }
  getModuleJson(e) {
    return this.getResModelJsonObj(e, this.moduleJsonSchemaPath);
  }
  getConfigJson(e) {
    return this.getResModelJsonObj(e, undefined);
  }
  setAppJson(e, o) {
    undefined === this.appJsonSchemaPath && this._log.printErrorExit("SCHEMA_PATH_NOT_INITIALIZED", [e]), this.setResModelJsonObj(e, o, this.appJsonSchemaPath);
  }
  setModuleJson(e, o) {
    undefined === this.moduleJsonSchemaPath && this._log.printErrorExit("SCHEMA_PATH_NOT_INITIALIZED", [e]), this.setResModelJsonObj(e, o, this.moduleJsonSchemaPath);
  }
  getResModelJsonObj(e, o) {
    const s = config_file_loader_js_1.configFileLoader.getConfigFileJson(e);
    if (s) return s;
    if (null !== s) {
      if (fs_extra_1.existsSync(e)) {
        const s = json_util_js_1.JsonUtil.getJson5Obj(e);
        return this.setResModelJsonObj(e, s, o), s;
      }
      return this._log.debug(`not found resModel json file in : ${e}`), void this.setResModelJsonObj(e, null, undefined);
    }
  }
  setResModelJsonObj(e, o, s) {
    s && this.validateBuildProfile(o, s), config_file_loader_js_1.configFileLoader.setConfigFileJson(e, o);
  }
  validateBuildProfile(e, o) {
    const s = validator_store_js_1.ValidatorStore.addValidator(o);
    try {
      validate_util_js_1.ValidateUtil.schemaCheckJSObject(e, undefined, s);
    } catch (e) {
      if (e instanceof Error) {
        if (hvigor_logger_1.ErrorUtil.isAdaptorError(e)) throw e;
        this._log.printErrorExit("SCHEMA_VALIDATE_FAILED", [`${e.name}: ${e.message}`], [["Check if the schema file exists.", "At file:", o]]);
      }
    }
  }
  initSchemaPath(e, o) {
    this.appJsonSchemaPath = e, this.moduleJsonSchemaPath = o;
  }
  clean() {
    this.appJsonSchemaPath = undefined, this.moduleJsonSchemaPath = undefined;
  }
}
export const resModelLoader = new ResModelLoader();
