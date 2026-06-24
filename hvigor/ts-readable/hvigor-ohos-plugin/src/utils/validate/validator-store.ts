/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/validator-store.js
 */
import ajv_1 from "ajv";
import fs_extra_1 from "fs-extra";
export class ValidatorStore {
  static getValidator(a) {
    return ValidatorStore.validatorMap.get(a);
  }
  static hasValidator(a) {
    return ValidatorStore.validatorMap.has(a);
  }
  static addValidator(a, t = false) {
    const r = `${a}:${t}`;
    if (ValidatorStore.hasValidator(r)) return ValidatorStore.getValidator(r);
    const e = fs_extra_1.readJSONSync(a);
    t && (e.properties.app.required = ["bundleName", "versionCode", "versionName"]);
    const o = new ajv_1({
      strict: false,
      allErrors: true
    }).compile(e);
    return ValidatorStore.validatorMap.set(r, o), o;
  }
  static clear() {
    ValidatorStore.validatorMap.clear();
  }
  static getValidatorMap() {
    return ValidatorStore.validatorMap;
  }
}
ValidatorStore.validatorMap = new Map();
