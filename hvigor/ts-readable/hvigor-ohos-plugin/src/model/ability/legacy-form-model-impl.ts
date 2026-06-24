/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/ability/legacy-form-model-impl.js
 */
import * as ability_type_enum_js_1 from "../../enum/ability-type-enum";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
export class LegacyFormModelImpl {
  constructor(e, t) {
    this._configObj = res_model_loader_js_1.resModelLoader.getConfigJson(e), this._jsObj = t, this._jsName = t.name;
  }
  getRelateSrcPath() {
    return this.getName();
  }
  getName() {
    return this._jsName;
  }
  getSrcLanguage() {
    return code_type_enum_js_1.CodeType.JS;
  }
  getType() {
    return ability_type_enum_js_1.AbilityTypeEnum.PAGE;
  }
  getConfigJsonJsObj() {
    return this._jsObj;
  }
}
