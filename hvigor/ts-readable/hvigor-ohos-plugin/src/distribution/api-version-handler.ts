/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/distribution/api-version-handler.js
 */
import * as sdk_const_js_1 from "../const/sdk-const";
import * as distro_filter_dimension_enum_js_1 from "../enum/distro-filter-dimension-enum";
import * as distro_filter_handler_js_1 from "./distro-filter-handler";
export class ApiVersionHandler extends distro_filter_handler_js_1.DistroFilterHandler {
  validateDistroFilterRequest(e, i) {
    this.doRequest(e, i, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.API_VERSION);
  }
  getDimensionValue(e) {
    return this.getCommonDimensionValue(e, this.getApiVersionScope(sdk_const_js_1.ApiVersion.DISTRO_FILTER_MIN_API_VERSION, sdk_const_js_1.ApiVersion.DISTRO_FILTER_MAX_API_VERSION), distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.API_VERSION, () => {
      const i = this.projectModel.getModuleModelByName(e.moduleName);
      this.logger._buildError("Invalid apiVersion value for distroFilter.")._detail("Ensure that the value of apiVersion for distroFilter is within the [3, 30] range.")._file(distro_filter_handler_js_1.DistroFilterHandler.getDetailFilePath(this.projectModel, i))._printErrorAndExit();
    });
  }
  getApiVersionScope(e, i) {
    const r = [];
    for (let s = e; s <= i; s++) r.push(s);
    return r;
  }
}
