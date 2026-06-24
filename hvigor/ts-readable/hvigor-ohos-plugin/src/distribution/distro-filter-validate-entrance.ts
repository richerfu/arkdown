/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/distribution/distro-filter-validate-entrance.js
 */
import * as common_const_js_1 from "../const/common-const";
import * as distro_filter_dimension_enum_js_1 from "../enum/distro-filter-dimension-enum";
import * as distro_filter_handler_js_1 from "./distro-filter-handler";
export class DistroFilterValidateEntrance extends distro_filter_handler_js_1.DistroFilterHandler {
  validateDistroFilterRequest(i, e) {
    const t = `The distroFilter settings are left empty in the entry ${distro_filter_handler_js_1.DistroFilterHandler.validateMultiTarget(this.projectModel) ? "executable targets" : "modules"} in ${common_const_js_1.CommonConst.PROFILE_JSON5} or ${common_const_js_1.CommonConst.CONFIG_JSON}.`;
    this.checkDistroFilterObjEmpty(i, e) ? this.logger._buildError(t)._detail("Select dimension: apiVersion, screenShape or screenWindow to configure and make sure the distroFilter settings can uniquely identify an entry module.")._printErrorAndExit() : this.nextDimensionHandler && this.nextDimensionHandler.validateDistroFilterRequest(i, e);
  }
  getDimensionValue(i) {}
  checkDistroFilterObjEmpty(i, e) {
    const t = this.checkDimensionConfigEmpty(i, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.API_VERSION) && this.checkDimensionConfigEmpty(e, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.API_VERSION),
      n = this.checkDimensionConfigEmpty(i, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_SHAPE) && this.checkDimensionConfigEmpty(e, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_SHAPE),
      s = this.checkDimensionConfigEmpty(i, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_WINDOW) && this.checkDimensionConfigEmpty(e, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_WINDOW);
    return t && n && s;
  }
}
