/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pre-build-app.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as api_version_handler_js_1 from "../distribution/api-version-handler";
import * as distro_filter_handler_js_1 from "../distribution/distro-filter-handler";
import * as distro_filter_validate_entrance_js_1 from "../distribution/distro-filter-validate-entrance";
import * as screen_shape_handler_js_1 from "../distribution/screen-shape-handler";
import * as screen_window_handler_js_1 from "../distribution/screen-window-handler";
import * as array_util_js_1 from "../utils/array-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as meta_util_js_1 from "../utils/meta-util";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_app_task_js_1 from "./task/ohos-app-task";
import * as inject_util_js_1 from "../utils/inject-util";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
const _log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-preBuildApp");
export class PreBuildApp extends ohos_app_task_js_1.OhosAppTask {
  constructor(e, t) {
    super(e, t, CommonTask.PRE_BUILD_APP);
  }
  doTaskAction() {
    this.checkConfigModuleStatus(), this.isApi8Fa(this.service) && this.validateDistroFilter(this.service.getProjectModel());
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.service.getProjectModel().isFaMode() && inject_util_js_1.InjectUtil.getAlignDeviceTypes().length && _log.warn('The "ohos.align.deviceTypes" does not take effect in FA mode.');
  }
  checkConfigModuleStatus() {
    if (undefined !== hvigor_1.hvigorCore.getExtraConfig().get("module")) {
      const e = this.service.getProjectModel().getModuleSpecificTargets().keys();
      for (const t of e) {
        const e = this.service.getProjectModel().getModuleModelByName(t);
        e && !e.isHarModule() && _log.printErrorExit("CANNOT_CONFIGURE_MODULE_WHEN_PACKING", [t]);
      }
    }
    if (0 === this.service.getProductDataMap().size) {
      const e = this.service.getTargetProduct().name;
      _log.printErrorExit("NO_HAP_FOUND", [this.project.getBuildProfilePath()], [[e]]);
    }
  }
  validateDistroFilter(e) {
    distro_filter_handler_js_1.DistroFilterHandler.validateMultiTarget(e) ? this.processTargetDistroFilter(e) : distro_filter_handler_js_1.DistroFilterHandler.validateMultiEntry(e) && this.processEntryDistroFilter(e);
  }
  isApi8Fa(e) {
    var t;
    const r = e.getTargetProduct(),
      s = null === (t = e.getProjectModel().getProductApiMeta(r.name)) || undefined === t ? undefined : t.compileSdkVersion;
    return e.isFaMode() && (null == s ? undefined : s.version) === sdk_const_js_1.ApiVersion.API_VERSION_8;
  }
  processTargetDistroFilter(e) {
    let t = [];
    const r = (r, s) => {
        r.forEach(r => {
          const i = distro_filter_handler_js_1.DistroFilterHandler.getTargetDistroFilterConfig(e, s, r);
          t.push(i);
        });
      },
      s = () => {
        t.length > 0 && (this.sendDistroFilterRequest(e, t), t = []);
      };
    e.getAllEntryModules().forEach(t => {
      const i = distro_filter_handler_js_1.DistroFilterHandler.getCurModuleExecutableTargets(e, t);
      i.length > 1 && distro_filter_handler_js_1.DistroFilterHandler.checkSingleEntryTargetDeviceType(e, t, i) && (r(i, t), s());
      distro_filter_handler_js_1.DistroFilterHandler.findIntersectedEntryList(e, t).forEach(o => {
        const n = distro_filter_handler_js_1.DistroFilterHandler.getCurModuleExecutableTargets(e, o);
        i.forEach(i => {
          n.forEach(n => {
            const l = distro_filter_handler_js_1.DistroFilterHandler.getTargetDeviceType(e, t, i),
              a = distro_filter_handler_js_1.DistroFilterHandler.getTargetDeviceType(e, o, n);
            (0, array_util_js_1.checkIntersection)(l, a) && (r([i], t), r([n], o)), s();
          });
        });
      });
    });
  }
  processEntryDistroFilter(e) {
    e.getAllEntryModules().forEach(t => {
      const r = distro_filter_handler_js_1.DistroFilterHandler.getCurModuleExecutableTargets(e, t)[0],
        s = distro_filter_handler_js_1.DistroFilterHandler.getTargetDeviceType(e, t, r);
      const i = distro_filter_handler_js_1.DistroFilterHandler.findIntersectedEntryList(e, t);
      0 !== i.length && this.handleIntersectedEntryList(i, e, s, t, []);
    });
  }
  handleIntersectedEntryList(e, t, r, s, i) {
    e.forEach(e => {
      const o = distro_filter_handler_js_1.DistroFilterHandler.getCurModuleExecutableTargets(t, e)[0],
        n = distro_filter_handler_js_1.DistroFilterHandler.getTargetDeviceType(t, e, o);
      if ((0, array_util_js_1.checkIntersection)(r, n)) {
        const r = distro_filter_handler_js_1.DistroFilterHandler.getFADistroFilterConfig(t, s);
        r && i.push(r);
        const o = distro_filter_handler_js_1.DistroFilterHandler.getFADistroFilterConfig(t, e);
        o && i.push(o), this.sendDistroFilterRequest(t, i), i = [];
      }
    });
  }
  sendDistroFilterRequest(e, t) {
    const r = new distro_filter_validate_entrance_js_1.DistroFilterValidateEntrance(e),
      s = new api_version_handler_js_1.ApiVersionHandler(e),
      i = new screen_shape_handler_js_1.ScreenShapeHandler(e),
      o = new screen_window_handler_js_1.ScreenWindowHandler(e);
    r.setNextDimensionHandler(s), s.setNextDimensionHandler(i), i.setNextDimensionHandler(o), r.executeValidate(t);
  }
  initTaskDepends() {
    (0, meta_util_js_1.hvigorOrToolChainsChanged)(this.service.getSdkInfo()) && this.dependsOn(CommonTask.CLEAN.name);
  }
}
