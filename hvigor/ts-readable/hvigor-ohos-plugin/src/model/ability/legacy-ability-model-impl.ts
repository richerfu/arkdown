/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/ability/legacy-ability-model-impl.js
 */
import * as ability_type_enum_js_1 from "../../enum/ability-type-enum";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class LegacyAbilityModelImpl {
  constructor(i, e) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyAbilityModelImpl.name), this._configObj = res_model_loader_js_1.resModelLoader.getConfigJson(i), this._configFile = i, this._abilityName = e, this._abilityObj = this.findAbilityByName(), this._jsObj = this.findJsByName();
  }
  findJsByName() {
    var i;
    if (!this._abilityObj || !this.hasJsObj()) return;
    const e = null !== (i = this._configObj.module.js) && undefined !== i ? i : [];
    for (const i of e) if (i.name === this._abilityName) return i;
    this._log.printErrorExit("PAGE_ABILITY_REQUIRE_JS_COMPONENT", [this._configFile], [[this._abilityName]]);
  }
  hasJsObj() {
    return this.getType() === ability_type_enum_js_1.AbilityTypeEnum.PAGE;
  }
  findAbilityByName() {
    var i;
    const e = null !== (i = this._configObj.module.abilities) && undefined !== i ? i : [];
    for (const i of e) if (i.name === this._abilityName) return i;
    throw this._log.printErrorExit("ABILITY_NAME_NOT_EXIST", [this._abilityName], [[this._configFile]]), new Error("Internal error.");
  }
  getRelateSrcPath() {
    const i = this._abilityObj.srcPath;
    return undefined === i ? this.getName() : i;
  }
  getName() {
    return this._abilityName;
  }
  getSrcLanguage() {
    return this._abilityObj.srcLanguage;
  }
  getType() {
    return ability_type_enum_js_1.AbilityTypeEnum.valueOf(this._abilityObj);
  }
  getConfigJsonJsObj() {
    return this._jsObj;
  }
}
