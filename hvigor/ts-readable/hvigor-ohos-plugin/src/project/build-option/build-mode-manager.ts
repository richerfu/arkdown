/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/build-option/build-mode-manager.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as build_mode_const_js_1 from "../../const/build-mode-const";
import * as common_const_js_1 from "../../const/common-const";
import * as task_names_js_1 from "../../tasks/common/task-names";
import * as array_util_js_1 from "../../utils/array-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as build_option_util_js_1 from "./build-option-util";
import * as build_option_util_js_2 from "./build-option-util";
import * as build_option_path_info_js_1 from "../../common/build-option-path-info";
import * as module_ohos_config_manager_js_1 from "../../common/global/module-ohos-config-manager";
import * as global_project_data_service_1 from "../../tasks/service/global-project-data-service";
import * as path_1 from "path";
const log = ohos_logger_js_1.OhosLogger.getLogger("BuildModeManager"),
  BUILD_OPTION_MERGE_ARRAY_KEYS = Object.freeze(["resOptions.compression.filters"]);
export class BuildModeManager {
  constructor() {
    this.productBuildOptionMap = new Map(), this.targetBuildOptionMap = {}, this.targetBuildOptionMapWithProductBuildMode = {};
  }
  initProjectBuildOptions(o, i) {
    var t, e, l;
    this.productBuildOptionMap.clear();
    const d = BuildModeManager.getBuildMode();
    let n = new Map();
    build_option_util_js_2.BuildOptionUtil.validateBuildMode(d, o) ? global_project_data_service_1.GlobalProjectDataService.getInstance().setBuildModeInfo({
      ifThrowError: true,
      buildModeName: d,
      buildProfilePath: o.getProfilePath()
    }) : global_project_data_service_1.GlobalProjectDataService.getInstance().setBuildModeInfo({
      ifThrowError: false,
      buildModeName: d,
      buildProfilePath: o.getProfilePath()
    });
    const u = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.DEBUGGABLE);
    log.debug(`Start initialize project's product build option map with build mode ${d}.`);
    const r = o.getProfileOpt(),
      _ = r.app.products;
    let a = null !== (t = r.app.buildModeSet) && undefined !== t ? t : [];
    a = build_option_util_js_2.BuildOptionUtil.arrayDeduplication(a);
    const s = null !== (l = null === (e = (0, array_util_js_1.getElementFromArr)(a, d)) || undefined === e ? undefined : e.buildOption) && undefined !== l ? l : {};
    _.forEach(t => {
      var e;
      let l = {};
      u && (s.debuggable = "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.DEBUGGABLE));
      const r = null !== (e = t.buildOption) && undefined !== e ? e : {};
      log.debug(`Picking option from product '${t.name}' with build mode '${d}'.`), l = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(r, l, BUILD_OPTION_MERGE_ARRAY_KEYS), n = build_option_util_js_1.BuildOptionCombiner.mergeBuildOptionPath(l, i, n), l = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(s, l, BUILD_OPTION_MERGE_ARRAY_KEYS), n = build_option_util_js_1.BuildOptionCombiner.mergeBuildOptionPath(l, i, n);
      const _ = build_option_util_js_2.BuildOptionUtil.getDefaultBuildModeBuildOpt(d);
      l = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(_, l, BUILD_OPTION_MERGE_ARRAY_KEYS), n = build_option_util_js_1.BuildOptionCombiner.mergeBuildOptionPath(l, i, n);
      const a = (0, path_1.dirname)(o.getProfilePath());
      this.convertCompressionPathToAbsolute(l, a), log.debug(`Product '${t.name}' build option initialized`), build_option_path_info_js_1.buildOptionPath.setProductOptPath(n, t.name), this.productBuildOptionMap.set(t.name, l);
    }), log.debug(`End initialize project's product build option map with build mode '${d}'.`);
  }
  initModuleBuildOptions(o, i) {
    this.initModuleBuildOptionMapWithProductBuildMode(o, i), this.initModuleBuildOptionMap(o, i, this.targetBuildOptionMap);
  }
  initModuleBuildOptionMapWithProductBuildMode(o, i) {
    const t = hvigor_1.hvigorCore.getExtraConfig().get("buildMode"),
      e = hvigor_1.hvigorCore.getExtraConfig().get("product"),
      l = o.getParentProject(),
      d = this.getAllBuildModes(l);
    for (let o of [build_mode_const_js_1.BuildModeConst.RELEASE, build_mode_const_js_1.BuildModeConst.DEBUG]) d.includes(o) || d.push(o);
    const n = l.getProductNames();
    for (let t of n) {
      this.targetBuildOptionMapWithProductBuildMode[t] || (this.targetBuildOptionMapWithProductBuildMode[t] = {});
      const e = this.targetBuildOptionMapWithProductBuildMode[t];
      for (let l of d) {
        e[l] || (e[l] = {});
        const d = hvigor_1.hvigorCore.getExtraConfig();
        d.set("product", t), d.set("buildMode", l), hvigor_1.hvigorCore.setExtraConfig(d), this.initModuleBuildOptionMap(o, i, e[l]);
      }
    }
    const u = hvigor_1.hvigorCore.getExtraConfig();
    e ? u.set("product", e) : u.delete("product"), t ? u.set("buildMode", t) : u.delete("buildMode"), hvigor_1.hvigorCore.setExtraConfig(u);
  }
  initModuleBuildOptionMap(o, i, t) {
    var e, l, d, n;
    const u = o.getName(),
      r = BuildModeManager.getBuildMode();
    t[u] ? t[u].clear() : t[u] = new Map();
    const _ = o.getParentProject().getProfileOpt(),
      a = build_option_util_js_2.BuildOptionUtil.arrayDeduplication(null !== (e = _.app.buildModeSet) && undefined !== e ? e : []),
      s = o.getProfileOpt(),
      p = build_option_util_js_2.BuildOptionUtil.getOrDefaultBuildModeBinderSet(null !== (l = s.buildModeBinder) && undefined !== l ? l : []);
    let g = null !== (d = s.buildOptionSet) && undefined !== d ? d : [];
    build_option_util_js_2.BuildOptionUtil.validateBuildMode(r, o.getParentProject()) ? global_project_data_service_1.GlobalProjectDataService.getInstance().setBuildModeInfo({
      ifThrowError: true,
      buildModeName: r,
      buildProfilePath: o.getParentProject().getProfilePath()
    }) : global_project_data_service_1.GlobalProjectDataService.getInstance().setBuildModeInfo({
      ifThrowError: false,
      buildModeName: r,
      buildProfilePath: o.getParentProject().getProfilePath()
    }), log.debug(`Start initialize module-target build option map, moduleName=${u}, buildMode=${r}`), g = build_option_util_js_2.BuildOptionUtil.arrayDeduplication(g), build_option_util_js_1.BuildOptionCombiner.processCopyFrom(g), o.getTargetOptions().forEach(e => {
      var l, d, n, _, c, b, m;
      let B = {},
        O = new Map();
      const h = (0, find_target_product_js_1.findTargetProduct)(o.getParentProject());
      hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.DEBUGGABLE) && (B.debuggable = "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.DEBUGGABLE));
      const M = module_ohos_config_manager_js_1.moduleOhosConfigManager.getBuildOption(u);
      B = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(M, B, BUILD_OPTION_MERGE_ARRAY_KEYS), O = build_option_util_js_1.BuildOptionCombiner.mergeBuildOptionPath(B, `${o.getModule().getBuildFilePath()}.ts`, O);
      const v = null !== (d = null === (l = e.config) || undefined === l ? undefined : l.buildOption) && undefined !== d ? d : {};
      log.debug(`Target '${e.name}' config initialized`), B = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(v, B, BUILD_OPTION_MERGE_ARRAY_KEYS), O = build_option_util_js_1.BuildOptionCombiner.mergeBuildOptionPath(B, i, O);
      const P = this.getBuildOptNameFromMapping(p, r, e.name),
        j = null !== (n = (0, array_util_js_1.getElementFromArr)(g, P)) && undefined !== n ? n : {};
      B = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(j, B, BUILD_OPTION_MERGE_ARRAY_KEYS), O = build_option_util_js_1.BuildOptionCombiner.mergeBuildOptionPath(B, i, O), B = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(null !== (_ = s.buildOption) && undefined !== _ ? _ : {}, B, BUILD_OPTION_MERGE_ARRAY_KEYS), O = build_option_util_js_1.BuildOptionCombiner.mergeBuildOptionPath(B, i, O);
      const E = (0, path_1.dirname)(o.getProfilePath());
      this.convertCompressionPathToAbsolute(B, E);
      const f = null !== (c = h.buildOption) && undefined !== c ? c : {};
      B = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(f, B, BUILD_OPTION_MERGE_ARRAY_KEYS);
      const C = build_option_path_info_js_1.buildOptionPath.getProductOptPath(h.name);
      O = build_option_util_js_1.BuildOptionCombiner.mergeProductBuildOptionPath(null != C ? C : new Map(), O);
      const A = null !== (m = null === (b = (0, array_util_js_1.getElementFromArr)(a, r)) || undefined === b ? undefined : b.buildOption) && undefined !== m ? m : {};
      B = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(A, B, BUILD_OPTION_MERGE_ARRAY_KEYS), O = build_option_util_js_1.BuildOptionCombiner.mergeBuildOptionPath(B, i, O), this.convertCompressionPathToAbsolute(B, o.getBelongProjectPath());
      const R = build_option_util_js_2.BuildOptionUtil.getDefaultModuleBuildOpt(P);
      B = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(R, B, BUILD_OPTION_MERGE_ARRAY_KEYS);
      const T = build_option_util_js_2.BuildOptionUtil.getDefaultBuildModeBuildOpt(r);
      B = build_option_util_js_1.BuildOptionCombiner.mergeBuildOption(T, B, BUILD_OPTION_MERGE_ARRAY_KEYS), build_option_path_info_js_1.buildOptionPath.setBuildOptionPath(u, e.name, O), t[u].set(e.name, B);
    }), null === (n = t[u]) || undefined === n || n.forEach((o, i) => {
      log.debug(`Module '${u}' target '${i}' build option initialized`);
    }), log.debug(`End initialize module-target build option map, moduleName=${u}`);
  }
  getBuildOptNameFromMapping(o, i, t) {
    var e, l;
    const d = null !== (l = (null !== (e = o.find(o => {
      if (o.buildModeName === i) return o;
    })) && undefined !== e ? e : {}).mappings) && undefined !== l ? l : [];
    for (const o of d) if (o.targetName === t) return o.buildOptionName;
    switch (log.debug(`Does not get target '${t}'corresponding build option.`), i) {
      case build_mode_const_js_1.BuildModeConst.DEBUG:
      case build_mode_const_js_1.BuildModeConst.TEST:
        return log.debug("Using preset build option 'debug'."), build_mode_const_js_1.BuildOptionConst.DEBUG;
      case build_mode_const_js_1.BuildModeConst.RELEASE:
        return log.debug("Using preset build option 'release'."), build_mode_const_js_1.BuildOptionConst.RELEASE;
      default:
        return log.debug("Using preset build option 'default'."), build_mode_const_js_1.BuildOptionConst.DEFAULT;
    }
  }
  getProductBuildOption(o) {
    var i;
    const t = null !== (i = this.productBuildOptionMap.get(o)) && undefined !== i ? i : {};
    return log.debug(`Obtain build option for product '${o}'in this build.`), t;
  }
  getTargetBuildOption(o, i) {
    var t, e;
    const l = null !== (e = null === (t = this.targetBuildOptionMap[o]) || undefined === t ? undefined : t.get(i)) && undefined !== e ? e : {};
    return log.debug(`Obtain build option for module '${o}' target '${i}'in this build.`), l;
  }
  getTargetBuildOptionWithProductBuildMode(o, i, t, e = "default") {
    var l, d;
    const n = null !== (d = null === (l = this.targetBuildOptionMapWithProductBuildMode[e][t][o]) || undefined === l ? undefined : l.get(i)) && undefined !== d ? d : {};
    return log.debug(`Module '${o}' target '${i}' using build option in ${t} buildMode.`), n;
  }
  static getBuildMode() {
    const o = hvigor_1.hvigorCore.getExtraConfig().get("buildMode");
    return o || (hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name) ? build_mode_const_js_1.BuildModeConst.RELEASE : build_mode_const_js_1.BuildModeConst.DEBUG);
  }
  getAllBuildModes(o) {
    var i, t;
    const e = o.getProfileOpt(),
      l = [];
    return null === (t = null === (i = null == e ? undefined : e.app) || undefined === i ? undefined : i.buildModeSet) || undefined === t || t.forEach(o => {
      l.push(null == o ? undefined : o.name);
    }), l;
  }
  convertCompressionPathToAbsolute(o, i) {
    var t, e, l;
    null === (l = null === (e = null === (t = o.resOptions) || undefined === t ? undefined : t.compression) || undefined === e ? undefined : e.filters) || undefined === l || l.forEach(o => {
      var t, e;
      (null === (t = o.files) || undefined === t ? undefined : t.path) && (o.files.path = this.convertPathToAbsolute(o.files.path, i)), (null === (e = o.exclude) || undefined === e ? undefined : e.path) && (o.exclude.path = this.convertPathToAbsolute(o.exclude.path, i));
    });
  }
  convertPathToAbsolute(o, i) {
    return o.map(o => (0, path_1.isAbsolute)(o) ? o : (0, path_1.resolve)(i, o));
  }
}
export const buildOptionManager = new BuildModeManager();
