/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-process-profile.js
 */
import * as fs from "fs-extra";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as legacy_merge_profile_js_1 from "./legacy-merge-profile";
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as json_util_js_1 from "../../utils/json-util";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
const INPUT_KEY = {
  arkEnable: "arkEnable",
  compileMode: "compileMode",
  deviceTypes: "deviceTypes",
  targetName: "targetName",
  isDebug: "isDebug"
};
export class LegacyProcessProfile extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, LegacyFATask.PROCESS_PROFILE), this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyProcessProfile.name), this._arkEnable = this.service.isArkModule(), this._deviceTypes = this.targetData.getTargetDeviceType(), this._intermediatesMergeLegacyProfile = this.pathInfo.getIntermediatesMergeLegacyProfile(), this._processedModuleJson = this.pathInfo.getIntermediatesProcessLegacyProfile(), this._moduleBuildProfilePath = this.moduleModel.getProfilePath();
  }
  initTaskDepends() {
    this.declareDepends(legacy_merge_profile_js_1.LegacyMergeProfile.name);
  }
  doTaskAction() {
    var e, t, i;
    const s = json_util_js_1.JsonUtil.getJson5Obj(this._intermediatesMergeLegacyProfile);
    if (this._arkEnable) {
      if (undefined !== s.module.distro) {
        const e = this.projectModel.getCompatibleApiMetaByProduct(this.targetData.getProduct().name).version;
        this._deviceTypes.includes(device_type_const_js_1.DeviceTypeConst.LITE_WEARABLE) || this._deviceTypes.includes(device_type_const_js_1.DeviceTypeConst.SMART_VISION) || (s.module.distro.virtualMachine = `ark${this.sdkInfo.getArkVersion(e, this.compatibleSdkVersionStage)}`);
      }
    }
    const o = this.moduleModel.getLegacyAbilities(this.targetData.getTargetName());
    this.validateTargetAbilityName(this.targetData.getTargetOpt(), o);
    const r = o.map(e => this.targetData.findTargetConfigOpt(e));
    this.replaceTargetAbilityPages(r, s), s.module.deviceType = this._deviceTypes;
    const a = null === (e = this.targetData.getTargetOpt().config) || undefined === e ? undefined : e.distroFilter;
    a && (s.module.distroFilter = a), this.moduleModel.isMixedDeviceModule() && this.targetData.hasRichDeviceInTarget() && (s.module.package = common_const_js_1.CommonConst.DEFAULT_PACKAGE_NAME, (null === (t = s.module.abilities) || undefined === t ? undefined : t.length) && ((null === (i = s.module.abilities[0].skills) || undefined === i ? undefined : i.length) ? s.module.abilities[0].skills = [...s.module.abilities[0].skills, {
      entities: ["entity.system.home"],
      actions: ["action.system.home"]
    }] : s.module.abilities[0].skills = [{
      entities: ["entity.system.home"],
      actions: ["action.system.home"]
    }])), fs.outputJSONSync(this._processedModuleJson, s);
  }
  replaceTargetAbilityPages(e, t) {
    var i;
    for (const s of e) undefined !== s && (null === (i = t.module.js) || undefined === i || i.forEach(e => {
      e.name === s.name && this.replaceAbilityPage(e, s);
    }));
  }
  replaceAbilityPage(e, t) {
    if (undefined === t.pages) return;
    const i = t.pages.filter(t => !e.pages.includes(t));
    0 === i.length ? e.pages = t.pages : this.printInvalidError("page", i.join(","));
  }
  validateTargetAbilityName(e, t) {
    var i, s, o, r;
    const a = null === (s = null === (i = null == e ? undefined : e.source) || undefined === i ? undefined : i.abilities) || undefined === s ? undefined : s.filter(e => !t.some(t => e.name === t.getName())).map(e => e.name);
    if (a && a.length > 0) return void this.printInvalidError("ability", a.join(","));
    const l = null === (r = null === (o = null == e ? undefined : e.source) || undefined === o ? undefined : o.abilities) || undefined === r ? undefined : r.filter(e => !t.some(t => e.name === t.getName() && "page" === t.getType())).map(e => e.name);
    l && l.length > 0 && this.printInvalidError("ability type, not page ability", l.join(","));
  }
  printInvalidError(e, t) {
    this._log.printErrorExit("INVALID_CONFIG_UNDER_TARGET", [e, this.moduleModel.getProfilePath()], [[t]]);
  }
  declareInputs() {
    const e = new Map();
    return e.set(INPUT_KEY.arkEnable, this._arkEnable), e.set(INPUT_KEY.deviceTypes, this._deviceTypes), e.set(INPUT_KEY.isDebug, this.targetService.isDebug()), e.set("compatibleSdkVersionStage", this.compatibleSdkVersionStage || ""), e;
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet();
    return e.addEntry(this._intermediatesMergeLegacyProfile), e.addEntry(this._moduleBuildProfilePath), e;
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntry(this._processedModuleJson);
  }
}
