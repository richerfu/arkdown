/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/distribution/screen-window-handler.js
 */
import * as distro_filter_dimension_enum_js_1 from "../enum/distro-filter-dimension-enum";
import * as distro_filter_handler_js_1 from "./distro-filter-handler";
export class ScreenWindowHandler extends distro_filter_handler_js_1.DistroFilterHandler {
  validateDistroFilterRequest(e, r) {
    this.doRequest(e, r, distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_WINDOW);
  }
  getDimensionValue(e) {
    var r;
    const i = null === (r = e.distroFilterObj) || undefined === r ? undefined : r.screenWindow;
    if (!i) return;
    const t = i.policy,
      o = i.value;
    if ("include" === t) return o;
    if ("exclude" === t) {
      const r = this.projectModel.getModuleModelByName(e.moduleName);
      this.logger._buildError("The exclude policy is not supported for screenWindow.")._file(distro_filter_handler_js_1.DistroFilterHandler.getDetailFilePath(this.projectModel, r))._printErrorAndExit();
    }
  }
}
