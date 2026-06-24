/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-merge-profile.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as fs from "fs-extra";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as merge_type_rule_js_1 from "../../enum/merge-type-rule";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as abstract_merge_profile_js_1 from "../abstract-merge-profile";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_pre_build_js_1 from "./legacy-pre-build";
import * as device_type_const_1 from "../../const/device-type-const";
import * as har_target_util_js_1 from "../../utils/har-target-util";
import * as json_util_js_1 from "../../utils/json-util";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
const JS_SHELL_PACKAGE = "com.example.myapplication";
export class LegacyMergeProfile extends abstract_merge_profile_js_1.AbstractMergeProfile {
  constructor(e) {
    super(e, LegacyFATask.MERGE_PROFILE), this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyMergeProfile.name), this._harLibs = this.mergeDependsLibs(common_const_js_1.CommonConst.CONFIG_JSON);
    const t = this.service.getModuleModel().getSourceSetByTargetName(this._targetName);
    this._moduleTargetRes = t.getLegacyModuleTargetRes(), this._mergedModuleJson = this._pathInfo.getIntermediatesMergeLegacyProfile();
  }
  initTaskDepends() {
    this.declareDepends(legacy_pre_build_js_1.LegacyPreBuild.name);
  }
  initHarModuleDepends() {
    har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).forEach((e, t) => this.declareDepends(`${e}@${LegacyFATask.MERGE_PROFILE.name}`, t));
  }
  mergeDslConfig(e) {
    var t, r, i;
    this._bundleName && (e.app.bundleName = this._bundleName, this._appBundleName = this._bundleName), this._vendor && (e.app.vendor = this._vendor, this._log.debug(`Change app vendor with '${this._vendor}'.`));
    const s = this.targetData.getApiMeta(),
      n = !![s.compileSdkVersion.version, s.compatibleSdkVersion.version, null !== (r = null === (t = s.targetSdkVersion) || undefined === t ? undefined : t.version) && undefined !== r ? r : s.compileSdkVersion.version].find(e => e <= sdk_const_js_1.ApiVersion.API_VERSION_9),
      o = null !== (i = s.targetSdkVersion) && undefined !== i ? i : s.compileSdkVersion,
      a = this.targetData.isHarmonyOS() ? this.apiTransform(o) : o.version,
      l = this.targetData.isHarmonyOS() ? this.apiTransform(s.compatibleSdkVersion) : s.compatibleSdkVersion.version,
      u = this.targetData.isHarmonyOS() ? common_const_js_1.CommonConst.HARMONY_OS : common_const_js_1.CommonConst.OPEN_HARMONY,
      c = this.sdkInfo.getToolchainsComponentVersion();
    e.app.apiVersion = {
      compileSdkVersion: c,
      compileSdkType: u,
      target: n ? o.version : a,
      compatible: n ? s.compatibleSdkVersion.version : l,
      releaseType: this.sdkInfo.getReleaseType()
    }, this._log.debug(`Change app target API version with '${e.app.apiVersion.target}'`), this._log.debug(`Change app minimum API version with '${e.app.apiVersion.compatible}'`), this._log.debug(`Change app releaseType with '${this.sdkInfo.getReleaseType()}'`);
  }
  doTaskAction() {
    const e = (0, wdk_1.cloneDeep)(this._moduleTargetRes.getConfigJsonOpt());
    this.mergeAsanAndTsan(e), this.mergeAppEnvironmentConfig(e), this._appBundleName = e.app.bundleName;
    const t = this._harLibs.map(e => fs.pathExistsSync(e) ? json_util_js_1.JsonUtil.getJson5Obj(e) : (this._log.warn(`${e} not found, The library will not be merged.\n         Check the configuration of this module.`), null)),
      r = this.mergeAllConfigOpt(e, t);
    if (this.mergeDslConfig(r), this.targetService.isDebug()) {
      undefined === r.deviceConfig.default && (r.deviceConfig.default = {}), r.deviceConfig.default.debug = true;
    }
    this.supportLiteDeviceModule(r) && !r.module.package && (r.module.package = JS_SHELL_PACKAGE), fs.outputJSONSync(this._mergedModuleJson, r, {
      spaces: "\t"
    });
  }
  mergeAsanAndTsan(e) {
    this.asanEnable && (e.app.asanEnabled = this.asanEnable), this.tsanEnable && (e.app.tsanEnabled = this.tsanEnable), this.hwAsanEnable && (e.app.hwasanEnabled = this.hwAsanEnable), this.ubsanEnable && (e.app.ubsanEnabled = this.ubsanEnable);
  }
  mergeAppEnvironmentConfig(e) {
    var t;
    const r = this.parseToAppEnvironments(this.appEnvironments);
    if (undefined !== e.app.appEnvironments && 0 !== e.app.appEnvironments.length) {
      this._log.debug("Change app appEnvironment");
      const i = new Map();
      r.forEach(e => {
        i.set(e.name, e.value);
      }), null === (t = e.app.appEnvironments) || undefined === t || t.forEach(e => {
        i.set(e.name, e.value);
      }), e.app.appEnvironments = Array.from(i, ([e, t]) => ({
        name: e,
        value: t
      }));
    } else this._log.debug("Use cli appEnvironment"), e.app.appEnvironments = r;
  }
  supportLiteDeviceModule(e) {
    const t = e.module.deviceType;
    return t && (t.includes(device_type_const_1.DeviceTypeConst.LITE_WEARABLE) || t.includes(device_type_const_1.DeviceTypeConst.SMART_VISION));
  }
  mergeAllConfigOpt(e, t) {
    var r, i, s, n, o;
    for (let a = 0; a < t.length; a++) {
      const l = t[a];
      null !== l && ((null === (r = l.app.apiVersion) || undefined === r ? undefined : r.compatible) && (null === (i = e.app.apiVersion) || undefined === i ? undefined : i.compatible) && (null === (s = l.app.apiVersion) || undefined === s ? undefined : s.compatible) > (null === (n = e.app.apiVersion) || undefined === n ? undefined : n.compatible) && this._log.printErrorExit("COMPATIBLE_VERSION_TOO_LOW", [l.module.name || (null === (o = l.module.distro) || undefined === o ? undefined : o.moduleName) || ""], [[l.app.apiVersion.compatible]]), e = this.mergeModel(e, l, "config", a === t.length - 1));
    }
    return e;
  }
  mergeModel(e, t, r, i) {
    if (LegacyMergeProfile.enumInclude(merge_type_rule_js_1.KeepHapOptions, r) || undefined === t) return e;
    if (undefined === e) return t;
    return (0, wdk_1.union)(Object.keys(e), Object.keys(t)).forEach(s => {
      const n = e[s],
        o = t[s];
      n && o && typeof n != typeof o && this._log.printErrorExit("TYPE_OF_VALUE_IN_CONFIG_IS_NOT_SAME", [s, this.moduleName]), this.checkFieldRule(e, t, s, r);
      switch (undefined === n ? typeof o : typeof n) {
        case "boolean":
          LegacyMergeProfile.handleBooleanMerge(e, n, o, s);
          break;
        case "string":
        case "number":
          LegacyMergeProfile.handleStringOrNumberMerge(e, n, o, s, r);
          break;
        case "object":
          Array.isArray(n) || Array.isArray(o) ? this.handleArrayMerge(e, n, o, s, r, i) : LegacyMergeProfile.enumInclude(merge_type_rule_js_1.IsMapFields, s) ? this.handleMapMerge(e, n, o, s, r, i) : e[s] = this.mergeModel(n, o, s, i);
          break;
        default:
          this._log.error("An unrecognized type!");
      }
    }), e;
  }
  mergeList(e, t, r, i) {
    let s = [];
    if ("skills" === r) return e;
    switch (0 === e.length ? typeof t[0] : typeof e[0]) {
      case "string":
        s = (0, wdk_1.union)(e, t);
        break;
      case "object":
        LegacyMergeProfile.enumInclude(merge_type_rule_js_1.HasPlaceholderOptions, r) && (e.forEach(e => this.disposePlaceHolder(e, r)), t.forEach(e => this.disposePlaceHolder(e, r))), e.forEach(e => {
          null === this.findObjectByUniqueKey(t, e, r) && s.push(e);
        }), t.forEach(t => {
          const n = this.findObjectByUniqueKey(e, t, r);
          if (null === n) s.push(t);else {
            const e = this.mergeModel(n, t, r, i);
            s.push(this.mergeRule(e, n));
          }
        });
        break;
      default:
        this._log.error(`Other Type! ${r}`);
    }
    if (i) for (const e of s) Object.keys(e).includes("mergeRule") && (e.mergeRule = undefined);
    return s;
  }
  findObjectByUniqueKey(e, t, r) {
    const i = LegacyMergeProfile.getUniqueKey(t, r);
    for (const s of e) {
      const e = LegacyMergeProfile.getUniqueKey(s, r);
      if ("abilities" === r) {
        const e = t.targetAbility,
          r = s.targetAbility;
        !e && r && e === i && this._log.printErrorExit("TARGET_ABILITY_NAME_CONFLICT", [e, this.moduleName]);
      }
      if (i === e) return s;
    }
    return null;
  }
  static getUniqueKey(e, t) {
    var r;
    if (LegacyMergeProfile.enumInclude(merge_type_rule_js_1.UniqueKeyEqualsName, t)) return e.name;
    switch (t) {
      case "shortcuts":
        return e.shortcutId;
      case "abilities":
        return null !== (r = e.targetAbility) && undefined !== r ? r : e.name;
      default:
        return null;
    }
  }
  mergeRule(e, t) {
    if (null === t || undefined === t.mergeRule) return e;
    const r = t.mergeRule;
    if (r.replace) {
      r.replace.forEach(r => {
        e[r] && (e[r] = t[r]);
      });
    }
    if (r.remove) {
      r.remove.forEach(t => {
        e[t] && (e[t] = undefined);
      });
    }
    return e;
  }
  mergeMap(e, t, r, i, s) {
    if (undefined === e) return e;
    if (undefined === t) return t;
    const n = (0, wdk_1.cloneDeep)(e),
      o = Object.keys(e),
      a = Object.keys(t),
      l = (0, wdk_1.difference)(a, o);
    r && l.forEach(e => {
      n[e] = t[e];
    });
    return (0, wdk_1.intersection)(a, o).forEach(r => {
      Array.isArray(e[r]) ? n[r] = (0, wdk_1.union)(e[r], t[r]) : "object" == typeof e[r] && (n[r] = this.mergeModel(e[r], t[r], i, s));
    }), n;
  }
  checkFieldRule(e, t, r, i) {
    if (!LegacyMergeProfile.enumInclude(merge_type_rule_js_1.IsNeedMergeRule, i) || LegacyMergeProfile.enumInclude(merge_type_rule_js_1.KeepAbilityMergeList, i) || "skills" === r) return;
    const s = e[r],
      n = t[r];
    if (undefined !== s && undefined !== n && !(0, wdk_1.isEqual)(s, n)) {
      const t = e.mergeRule;
      if (!(0, wdk_1.union)(t.replace, t.remove).includes(r)) {
        const e = LegacyMergeProfile.getUniqueKey(s, i);
        this._log.printErrorExit("MERGE_RULE_CONFLICT", [i, e, r, this.moduleName]);
      }
    }
  }
  disposePlaceHolder(e, t) {
    return Object.keys(e).forEach(r => {
      !LegacyMergeProfile.enumInclude(merge_type_rule_js_1.NeedDisposePlaceholder, r) || "abilities" === t && "name" === r || this.replaceAppBundleName(e, r);
    }), e;
  }
  replaceAppBundleName(e, t) {
    var r;
    const i = e[t];
    if ("string" == typeof i) e[t] = i.replace("{bundleName}", this._appBundleName);else if (Array.isArray(i) && 0 !== i.length) if ("string" == typeof i[0]) {
      const r = [];
      for (const i of e[t]) r.push(i.replace("{bundleName}", this._appBundleName));
      e[t] = r;
    } else {
      const i = [];
      for (const s of e[t]) s.name = null === (r = s.name) || undefined === r ? undefined : r.replace("{bundleName}", this._appBundleName), i.push(s);
      e[t] = i;
    }
  }
  static enumInclude(e, t) {
    return Object.values(e).includes(t);
  }
  static handleBooleanMerge(e, t, r, i) {
    LegacyMergeProfile.enumInclude(merge_type_rule_js_1.KeepOneOfThem, i) && (e[i] = undefined === t && undefined !== r ? r : t);
  }
  static handleStringOrNumberMerge(e, t, r, i, s) {
    let n = undefined === t && undefined !== r ? r : t;
    "deviceConfig" !== s && "module" !== s || (n = t), e[i] = n;
  }
  handleArrayMerge(e, t, r, i, s, n) {
    const o = Array.isArray(t) ? t : [],
      a = Array.isArray(r) ? r : [];
    let l;
    l = "module" === s && LegacyMergeProfile.enumInclude(merge_type_rule_js_1.KeepModuleAttr, i) ? [...o] : this.mergeList(o, a, i, n), 0 !== l.length && (e[i] = l);
  }
  handleMapMerge(e, t, r, i, s, n) {
    let o = true;
    "config" === s && (o = false, undefined === t.default && undefined !== r.default && (undefined === r.default.allowComponentsProxy ? r.default = undefined : t.default = {})), e[i] = this.mergeMap(t, r, o, i, n);
  }
  declareInputs() {
    var e;
    const t = super.declareInputs(),
      r = null === (e = this.targetData.getApiMeta().targetSdkVersion) || undefined === e ? undefined : e.version;
    return r && t.set("targetSdkVersion", r), t;
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet();
    return e.addEntry(this._moduleTargetRes.getJsonPath()), this._harLibs.forEach(t => {
      fs.existsSync(t) && e.addEntry(t);
    }), e;
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntry(this._mergedModuleJson);
  }
}
