/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/validate-config.js
 */
import * as common_const_js_1 from "../../const/common-const";
import * as validate_util_js_1 from "./validate-util";
import * as validator_store_js_1 from "./validator-store";
export function validateOhosProjectConfig(o, t) {
  const a = validator_store_js_1.ValidatorStore.addValidator(common_const_js_1.ConfigSchemaFileConst.PROJECT_SCHEMA_PATH, false);
  validate_util_js_1.ValidateUtil.schemaCheckJSObject(o, t, a);
}
export function validateOhosModuleConfig(o, t) {
  const a = validator_store_js_1.ValidatorStore.addValidator(common_const_js_1.ConfigSchemaFileConst.MODULE_SCHEMA_PATH, false);
  validate_util_js_1.ValidateUtil.schemaCheckJSObject(o, t, a);
}
export function validateOhosHarModuleConfig(o, t) {
  const a = validator_store_js_1.ValidatorStore.addValidator(common_const_js_1.ConfigSchemaFileConst.HAR_MODULE_SCHEMA_PATH, false);
  validate_util_js_1.ValidateUtil.schemaCheckJSObject(o, t, a);
}
