/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-make-pack-info.js
 */
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_merge_profile_js_1 from "./legacy-merge-profile";
import * as hvigor_1 from "@ohos/hvigor";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as abstract_make_pack_info_js_1 from "../abstract-make-pack-info";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyMakePackInfo extends abstract_make_pack_info_js_1.AbstractMakePackInfo {
  declareInputFiles() {
    const e = this.targetService.getTargetData().getModuleSourceSetModel().getLegacyModuleTargetRes().getJsonPath();
    return new hvigor_1.FileSet().addEntry(e);
  }
  constructor(e) {
    super(e, LegacyFATask.MAKE_PACK_INFO), this.log = ohos_logger_js_1.OhosLogger.getLogger(LegacyMakePackInfo.name);
  }
  initTaskDepends() {
    this.declareDepends(legacy_merge_profile_js_1.LegacyMergeProfile.name);
  }
  doTaskAction() {
    const e = this.moduleModel.getJsonObjByTargetName(this.targetName),
      t = {
        bundleName: e.app.bundleName,
        version: e.app.version
      };
    this.doMakePackInfo(this.log, t, e.app.apiVersion);
  }
  getModuleObj(e, t, a, s) {
    const i = t.getJsonObjByTargetName(s);
    return {
      mainAbility: i.module.mainAbility,
      deviceType: e,
      abilities: this.processAbilities(i.module.abilities),
      distro: i.module.distro,
      apiVersion: null != a ? a : i.app.apiVersion
    };
  }
  getPackageObj(e, t, a, s) {
    if (this.targetData.isSingleDeviceTypeTarget()) return [{
      deviceType: e.getTargetDeviceType(),
      moduleType: t.module.distro.moduleType,
      deliveryWithInstall: t.module.distro.deliveryWithInstall,
      name: a
    }];
    const i = this.targetData.getTargetDeviceType().filter(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e));
    return [{
      deviceType: this.targetData.getTargetDeviceType().filter(e => !device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e)),
      moduleType: t.module.distro.moduleType,
      deliveryWithInstall: t.module.distro.deliveryWithInstall,
      name: a
    }, {
      deviceType: i,
      moduleType: t.module.distro.moduleType,
      deliveryWithInstall: t.module.distro.deliveryWithInstall,
      name: s
    }];
  }
}
