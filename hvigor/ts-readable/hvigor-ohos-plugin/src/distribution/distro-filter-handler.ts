/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/distribution/distro-filter-handler.js
 */
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as distro_filter_dimension_enum_js_1 from "../enum/distro-filter-dimension-enum";
import * as array_util_js_1 from "../utils/array-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as common_util_js_1 from "../common/common-util";
import * as find_target_product_js_1 from "../common/find-target-product";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as json_util_js_1 from "../utils/json-util";
import * as res_model_loader_js_1 from "../utils/loader/file/res-model-loader";
export class DistroFilterHandler {
  constructor(e, t) {
    this.logger = ohos_logger_js_1.OhosLogger.getLogger(DistroFilterHandler.name), this.nextDimensionHandler = t, this.projectModel = e;
  }
  setNextDimensionHandler(e) {
    this.nextDimensionHandler = e;
  }
  executeValidate(e) {
    this.checkDistroFilterLabel();
    for (let t = 0; t < e.length; t++) for (let i = e.length - 1; i > t; i--) this.validateDistroFilterRequest(e[t], e[i]);
  }
  doRequest(e, t, i) {
    const r = e.moduleName,
      o = t.moduleName,
      s = this.getDimensionValue(e),
      l = this.getDimensionValue(t),
      n = undefined === s,
      a = undefined === l,
      d = n ? r : o,
      u = DistroFilterHandler.getCurModuleExecutableTargetNameList(this.projectModel, d),
      c = DistroFilterHandler.validateMultiTarget(this.projectModel);
    if (n !== a) {
      const e = c ? "executable targets" : "modules",
        t = c ? u : d,
        r = this.projectModel.getModuleModelByName(d),
        o = DistroFilterHandler.getDetailFilePath(this.projectModel, r);
      this.logger._buildError(`${i} is not configured for all ${e}.`)._detail(`Ensure that ${i} is configured for all ${e} [${t}].`)._file(o)._printErrorAndExit();
    }
    if (this.checkDimensionConfigEmpty(e, i) && this.checkDimensionConfigEmpty(t, i) || (0, array_util_js_1.checkIntersection)(s, l)) {
      const i = DistroFilterHandler.getCurModuleExecutableTargetNameList(this.projectModel, r),
        s = DistroFilterHandler.getCurModuleExecutableTargetNameList(this.projectModel, o),
        l = c ? `The configured distroFilter in all executable targets [${i}] in '${r}', [${s}] in '${o}' settings overlap.` : `The configured distroFilter in '${r}' and '${o}' settings overlap.`;
      if (!this.nextDimensionHandler) return void this.logger._buildError(l)._detail("Ensure that the distroFilter settings do not overlap or add new dimensions for distroFilter.")._printErrorAndExit();
      this.nextDimensionHandler.validateDistroFilterRequest(e, t);
    }
  }
  getDistroFilterObjByDimension(e, t) {
    if (e) switch (t) {
      case distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.API_VERSION:
        return e.apiVersion;
      case distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_SHAPE:
        return e.screenShape;
      case distro_filter_dimension_enum_js_1.DistroFilterDimensionEnum.SCREEN_WINDOW:
        return e.screenWindow;
      default:
        return;
    }
  }
  static getFADistroFilterConfig(e, t) {
    var i, r, o, s;
    const l = this.getCurModuleExecutableTargets(e, t);
    if (null === (r = null === (i = l[0].target) || undefined === i ? undefined : i.config) || undefined === r ? undefined : r.distroFilter) return {
      moduleName: t,
      distroFilterObj: null === (s = null === (o = l[0].target) || undefined === o ? undefined : o.config) || undefined === s ? undefined : s.distroFilter
    };
    const n = this.getConfigOptObj(e, t).module.distroFilter;
    return undefined === n ? undefined : {
      moduleName: t,
      distroFilterObj: n
    };
  }
  static getCurModuleExecutableTargets(e, t) {
    const i = (0, find_target_product_js_1.findTargetProduct)(e),
      r = e.getProfileOpt(),
      o = [];
    return r.modules.forEach(r => {
      var s;
      if (r.name !== t) return;
      const l = e.getModuleModelByName(t),
        n = null == l ? undefined : l.getTargetOptions();
      null === (s = r.targets) || undefined === s || s.forEach(e => {
        const t = null == n ? undefined : n.find(t => t.name === e.name);
        e.applyToProducts.forEach(e => {
          if (e === i.name) {
            const e = {
              moduleName: r.name,
              target: t
            };
            o.push(e);
          }
        });
      });
    }), o;
  }
  static getCurModuleExecutableTargetNameList(e, t) {
    const i = DistroFilterHandler.getCurModuleExecutableTargets(e, t),
      r = [];
    return i.forEach(e => {
      e.target && r.push(e.target.name);
    }), r;
  }
  static getCurExecutableTargetsDistroFilterConfigList(e) {
    const t = [];
    return e.getAllEntryModules().forEach(i => {
      const r = this.getCurModuleExecutableTargets(e, i);
      r.length > 1 && r.forEach(r => {
        const o = this.getTargetDistroFilterConfig(e, i, r);
        t.push(o);
      });
    }), t;
  }
  getCommonDimensionValue(e, t, i, r) {
    const o = e.moduleName,
      s = this.projectModel.getModuleModelByName(o),
      l = DistroFilterHandler.getDetailFilePath(this.projectModel, s),
      n = DistroFilterHandler.getCurModuleExecutableTargetNameList(this.projectModel, o);
    e.distroFilterObj || this.logger._buildError("DistroFilter must be configured for all executable targets of the entry module.")._detail(`The distroFilter is missing for the executable targets [${n}] of the ${o}.`)._file(l)._printErrorAndExit();
    const a = this.getDistroFilterObjByDimension(e.distroFilterObj, i);
    if (!a) return;
    const d = a.policy,
      u = a.value;
    return u || this.logger._buildError(`The ${i} value configuration is incorrect.`)._detail(`Please check whether the distroFilter configuration in module '${o}' is correct.`)._file(l)._printErrorAndExit(), u.forEach(e => {
      t.includes(e) || r();
    }), "include" === d && (0, array_util_js_1.isSubset)(t, u) ? u : "exclude" === d ? (0, array_util_js_1.complementarySet)(t, u) : undefined;
  }
  static getDetailFilePath(e, t) {
    var i, r;
    if (!t) return "";
    const o = e.getProjectDir(),
      s = t.getModule().getNodeDir(),
      l = this.getCurModuleExecutableTargets(e, s),
      n = 1 === l.length && (null === (r = null === (i = l[0].target) || undefined === i ? undefined : i.config) || undefined === r ? undefined : r.distroFilter);
    return path_1.resolve(o, s, this.validateMultiTarget(e) || n ? `${common_const_js_1.CommonConst.PROFILE_JSON5}` : `src/main/${common_const_js_1.CommonConst.CONFIG_JSON}`);
  }
  static getTargetDeviceType(e, t, i) {
    var r, o, s;
    return (null === (o = null === (r = i.target) || undefined === r ? undefined : r.config) || undefined === o ? undefined : o.deviceType) ? null === (s = i.target) || undefined === s ? undefined : s.config.deviceType : this.getConfigOptObj(e, t).module.deviceType;
  }
  static getTargetDistroFilterConfig(e, t, i) {
    var r, o;
    const s = this.getConfigOptObj(e, t);
    return (null === (o = null === (r = i.target) || undefined === r ? undefined : r.config) || undefined === o ? undefined : o.distroFilter) ? {
      moduleName: t,
      distroFilterObj: i.target.config.distroFilter
    } : {
      moduleName: t,
      distroFilterObj: s.module.distroFilter
    };
  }
  static getConfigOptObj(e, t) {
    const i = path_1.resolve(e.getProjectDir(), t, `src/main/${common_const_js_1.CommonConst.CONFIG_JSON}`);
    return res_model_loader_js_1.resModelLoader.getConfigJson(i);
  }
  static getDeviceTypeList(e) {
    const t = e.getAllEntryModules(),
      i = [];
    return t.forEach(t => {
      const r = e.getModuleModelByName(t);
      if (undefined === r) return;
      const o = r.getDeviceTypes();
      i.push(o);
    }), i;
  }
  checkDimensionConfigEmpty(e, t) {
    return undefined === this.getDistroFilterObjByDimension(e.distroFilterObj, t);
  }
  checkDistroFilterLabel() {
    let e = "",
      t = "";
    const i = (0, find_target_product_js_1.findTargetProduct)(this.projectModel);
    this.projectModel.getAllEntryModules().forEach(r => {
      const o = this.projectModel.getModuleModelByName(r),
        s = DistroFilterHandler.getCurModuleExecutableTargetNameList(this.projectModel, r),
        l = DistroFilterHandler.getDetailFilePath(this.projectModel, o);
      if (DistroFilterHandler.validateMultiTarget(this.projectModel)) DistroFilterHandler.checkExistTargetDistroFilter(this.projectModel) || (e = `When multiple targets in the current entry module are applied to the current product, the executable targets in ${common_const_js_1.CommonConst.PROFILE_JSON5} file must contain the distroFilter label, and the settings of this label must be able to uniquely identify an target.`, t = `Add the distroFilter label under target-config tag in all executable targets [${s}] which applied to '${i.name}' product in the '${r}' ${common_const_js_1.CommonConst.PROFILE_JSON5} file.`, this.logger._buildError(e)._detail(t)._file(l)._printErrorAndExit());else if (DistroFilterHandler.validateMultiEntry(this.projectModel)) {
        const i = DistroFilterHandler.findIntersectedEntryList(this.projectModel, r);
        if (0 === i.length) return;
        e = `When multiple entry modules exist the same device type, the ${common_const_js_1.CommonConst.CONFIG_JSON} or ${common_const_js_1.CommonConst.PROFILE_JSON5} file must contain the distroFilter label, and the settings of this label must be able to uniquely identify an entry module.`, t = `Check the distroFilter label under module tag in the '${r}' and '${i}' ${common_const_js_1.CommonConst.CONFIG_JSON} file or their target config in ${common_const_js_1.CommonConst.PROFILE_JSON5} file.`;
        DistroFilterHandler.getFADistroFilterConfig(this.projectModel, r) || this.logger._buildError(e)._detail(t)._file(l)._printErrorAndExit();
      }
    });
  }
  static checkExistTargetDistroFilter(e) {
    return this.getCurExecutableTargetsDistroFilterConfigList(e).some(e => undefined !== e.distroFilterObj);
  }
  static validateMultiTarget(e) {
    for (const t of e.getAllEntryModules()) {
      if (DistroFilterHandler.getCurModuleExecutableTargets(e, t).length > 1) return true;
    }
    return false;
  }
  static findIntersectedEntryList(e, t) {
    const i = e.getModuleModelByName(t),
      r = [];
    if (!i) return [];
    const o = i.getDeviceTypes(),
      s = e.getAllEntryModules();
    s.delete(t);
    for (const t of s) {
      const i = e.getModuleModelByName(t);
      if (!i) continue;
      const s = i.getDeviceTypes();
      (0, array_util_js_1.checkIntersection)(o, s) && r.push(t);
    }
    return r;
  }
  static checkSingleEntryTargetDeviceType(e, t, i) {
    const r = new Set();
    for (const o of i) {
      const i = DistroFilterHandler.getTargetDeviceType(e, t, o);
      for (const e of i) {
        if (r.has(e)) return true;
        r.add(e);
      }
    }
    return false;
  }
  static validateMultiEntry(e) {
    const t = new Set();
    for (const i of this.getDeviceTypeList(e)) for (const e of i) {
      if (t.has(e)) return true;
      t.add(e);
    }
    return false;
  }
  static findFirstDistroFilterConfigMetadata(e, t, i) {
    if (!i) return;
    const r = t.getModule().getNodeDir();
    return i.find(t => {
      if (t.resource) {
        const i = (0, common_util_js_1.parsingProfileName)(t.resource),
          o = path_1.resolve(e, r, `src/main/${build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR}/${i}.json`),
          s = json_util_js_1.JsonUtil.getJson5Obj(o);
        return s.distroFilter || s.distributionFilter;
      }
    });
  }
  static checkMetadataExistDistroFilterConfig(e, t, i) {
    return !!i && undefined !== this.findFirstDistroFilterConfigMetadata(e, t, i);
  }
}
