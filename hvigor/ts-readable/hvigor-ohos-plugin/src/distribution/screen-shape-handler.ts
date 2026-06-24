/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/distribution/screen-shape-handler.js
 */
import * as distro_filter_dimension_enum_js_1 from "../enum/distro-filter-dimension-enum";
import * as distro_filter_handler_js_1 from "./distro-filter-handler";
export class ScreenShapeHandler extends distro_filter_handler_js_1.DistroFilterHandler {
  validateDistroFilterRequest(e, r) {
    this.doRequest(e, r, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_SHAPE);
  }
  getDimensionValue(e) {
    return this.getCommonDimensionValue(e, ["circle", "rect"], distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_SHAPE, () => {
      const r = this.projectModel.getModuleModelByName(e.moduleName);
      this.logger._buildError("Invalid screenShape value for distroFilter.")._detail('Ensure that the value of screenShape for distroFilter is within the ["circle", "rect"] range.')._file(distro_filter_handler_js_1.DistroFilterHandler.getDetailFilePath(this.projectModel, r))._printErrorAndExit();
    });
  }
}
