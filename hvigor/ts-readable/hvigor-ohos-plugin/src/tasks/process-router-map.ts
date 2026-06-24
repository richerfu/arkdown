/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-router-map.js
 */
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as array_util_js_1 from "../utils/array-util";
import * as dependency_util_js_1 from "../utils/dependency-util";
import * as file_util_js_1 from "../utils/file-util";
import * as har_target_util_js_1 from "../utils/har-target-util";
import * as hsp_target_utils_js_1 from "../utils/hsp-target-utils";
import * as json_util_js_1 from "../utils/json-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ohmurl_utils_js_1 from "../utils/ohmurl-utils";
import * as task_names_js_1 from "./common/task-names";
import * as deduplicate_util_js_1 from "./har-deduplicate/deduplicate-util";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
export class ProcessRouterMap extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    var t, a, s;
    super(e, task_names_js_1.TaskNames.Task.PROCESS_ROUTER_MAP), this._log = ohos_logger_js_1.OhosLogger.getLogger(ProcessRouterMap.name), this.moduleName2Target = new Map(), this.compileEntry = [], this.routerMapFileName = this.targetService.getRouterMapJsonFileName(this.moduleModel), this.intermediatesRouterMapObjs = [], this.intermediatesTempRouterMapFilePath = this.pathInfo.getIntermediatesRouterMap(null !== (t = this.routerMapFileName) && undefined !== t ? t : build_directory_const_js_1.BuildArtifactConst.TEMP_ROUTER_MAP_FILE_NAME), this.intermediatesToLoaderRouterMap = this.pathInfo.getIntermediatesRouterMap(build_directory_const_js_1.BuildArtifactConst.LOADER_ROUTER_MAP_FILE_NAME), this.useNormalizedOHMUrl = !!(null === (s = null === (a = this.targetService.getBuildOption()) || undefined === a ? undefined : a.strictMode) || undefined === s ? undefined : s.useNormalizedOHMUrl), this.initHarDeduplicateData();
  }
  initHarDeduplicateData() {
    if (this.targetService.shouldDoDeduplicateTask()) if (this.moduleModel.isHapModule()) {
      const e = (0, collect_duplicate_har_dependencies_js_1.getMergedHapAllHspDependencies)();
      this.allHspDepTargets = hsp_target_utils_js_1.HspTargetUtils.calHspTargets(this.targetService, e);
    } else this.moduleModel.isHspModule() && (this.hspNeedDeduplicateHarInfo = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName));
  }
  declareInputFiles() {
    const e = super.declareInputFiles(),
      t = this.projectModel.isOhpmProject();
    e.addEntry(t ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath()).addEntry(t ? this.projectModel.getOhPackageJson5Path() : this.projectModel.getPackageJsonPath()).addEntry(this.moduleModel.getJsonPathByTargetName(this.targetName));
    const a = this.targetService.getRouterMapJsonPath(this.moduleModel);
    return a && fs_extra_1.existsSync(a) && e.addEntry(a), fs_extra_1.existsSync(this.pathInfo.getIntermediatesPkgContextInfoPath()) && e.addEntry(this.pathInfo.getIntermediatesPkgContextInfoPath()), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule() && this.processHarDeduplicateInputFiles(e), e;
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles().addEntry(this.intermediatesTempRouterMapFilePath).addEntry(this.intermediatesToLoaderRouterMap);
    return this.hspNeedDeduplicateHarInfo && Object.keys(this.hspNeedDeduplicateHarInfo).length > 0 && (e.addEntry(this.pathInfo.getRouterMapDuplicateInfoPath()), e.addEntry(this.pathInfo.getLoaderRouterMapDuplicateInfoPath())), e;
  }
  declareInputs() {
    const e = super.declareInputs();
    e.set(common_const_js_1.CommonConst.USE_NORMALIZED_OHM_URL, this.useNormalizedOHMUrl), e.set("obfuscated", this.targetService.isOpenSource()), e.set("byteCodeHar", this.targetService.isByteCodeHar()), e.set("bundleName", this.getBundleName()), e.set("moduleName", this.moduleName);
    const t = [],
      a = [],
      s = [];
    return this.service.getHarDependencies().forEach(e => {
      if (t.push(e.getPackageName()), e.isLocal()) {
        const t = this.getLocalDependencyRouterMap(e);
        t && a.push(t);
      } else {
        const t = dependency_manager_js_1.DependencyManager.getRemoteDependencyRouterMapObjList(e);
        t && s.push(t);
      }
    }), e.set("harPackageNameList", JSON.stringify(t)), e.set("localDependencyRouterMapObjArray", (0, hvigor_common_1.hash)(JSON.stringify(a))), e.set("remoteDependencyRouterMapObjArray", (0, hvigor_common_1.hash)(JSON.stringify(s))), e;
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.moduleName2Target = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService);
    const e = this.targetService.getTargetRouterMapObjList(this.moduleModel);
    this.doSelfRouterMapNameCheck(e);
  }
  doSelfRouterMapNameCheck(e) {
    if (!(null == e ? undefined : e.length)) return;
    const t = new Set(),
      a = new Set();
    e.forEach(e => {
      const s = e.name;
      t.has(s) ? a.add(s) : t.add(s);
    }), a.size && this._log.warn(`Duplicate 'routerMap' object names detected: '${[...a].join(",")}'. Please make sure the names are unique in the current module.`);
  }
  doTaskAction() {
    const e = this.targetService.getTargetRouterMapObjList(this.moduleModel);
    this.service.getModuleModel().getModuleType() !== module_type_enum_js_1.ModuleType.Har || this.targetService.isBundledDependencies() ? this.processHapOrHspRouterMap(e) : this.processHarRouterMap(e);
  }
  processHarRouterMap(e) {
    if (!e) return;
    this.checkModuleRouterMapObj(e, this.moduleModel, false), this.checkRouterMapObjName(e);
    const t = this.hasUseTsHarField() ? ".ts" : ".js";
    e.forEach(e => {
      const a = {
        name: e.name,
        pageSourceFile: path_1.resolve(this.module.getNodeDir(), e.pageSourceFile),
        buildFunction: e.buildFunction
      };
      if (this.intermediatesRouterMapObjs.push(a), this.compileEntry.push(a.pageSourceFile), !this.targetService.isOpenSource() || this.targetService.isByteCodeHar()) {
        const a = e.pageSourceFile,
          s = file_util_js_1.FileUtil.getFileSuffix(a);
        common_const_js_1.ValidateRegExp.DYNAMIC_IMPORT_FILE_SUFFIX_REG_EXP.test(s) && Object.assign(e, {
          pageSourceFile: `${a.substring(0, a.lastIndexOf("."))}${t}`,
          originalSuffix: `.${s}`
        });
      }
    }), this.generateTempRouterMap(e, this.intermediatesRouterMapObjs);
  }
  processHapOrHspRouterMap(e) {
    const t = [];
    this.processModuleSelfRouterMap(t, e), this.processModuleDependencyRouterMap(t), this.processHarDeduplicate(t, this.intermediatesRouterMapObjs), this.checkRouterMapObjName(t), this.generateTempRouterMap(t, this.intermediatesRouterMapObjs);
  }
  processModuleSelfRouterMap(e, t) {
    t && (this.checkModuleRouterMapObj(t, this.moduleModel, false), t.forEach(e => {
      const t = path_1.resolve(this.moduleModel.getModule().getNodeDir(), e.pageSourceFile);
      this.compileEntry.push(t);
      let a = "";
      if (this.useNormalizedOHMUrl) {
        const t = this.pathInfo.getIntermediatesPkgContextInfoPath(),
          s = json_util_js_1.JsonUtil.getJson5Obj(t)[this.packageJsonObj.name];
        a = (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(s, e.pageSourceFile);
      } else a = this.generateModuleOhmurl(this.moduleName, e.pageSourceFile);
      Object.assign(e, {
        ohmurl: a,
        bundleName: this.getBundleName(),
        moduleName: this.moduleName
      });
      const s = {
        ohmurl: a,
        name: e.name,
        pageSourceFile: t,
        buildFunction: e.buildFunction
      };
      this.intermediatesRouterMapObjs.push(s);
    }), e.push(...t));
  }
  processModuleDependencyRouterMap(e) {
    const t = new Map();
    this.service.getHarDependencies().forEach(a => {
      a.isLocal() ? this.processLocalDependencyRouterMap(a, e, t) : this.processRemoteDependencyRouterMap(a, e);
    });
  }
  getLocalDependencyRouterMap(e) {
    return (0, dependency_util_js_1.getLocalDependencyRouterMap)(this.projectModel, this.targetService, e);
  }
  processLocalDependencyRouterMap(e, t, a) {
    const s = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(e, this.projectModel),
      r = this.getLocalDependencyRouterMap(e);
    s && r && !a.has(s) && (a.set(s, r), this.checkModuleRouterMapObj(r, s, true), r.forEach(t => {
      const a = path_1.resolve(e.getDependencyRootPath(), t.pageSourceFile),
        r = s.getModuleType();
      this.pushCompileEntry(r, a);
      let o = "";
      this.useNormalizedOHMUrl ? o = this.getOhmUrlWithNormalize(e, t.pageSourceFile) : (r === module_type_enum_js_1.ModuleType.Shared && (o = this.generateModuleOhmurl(s.getName(), t.pageSourceFile)), r === module_type_enum_js_1.ModuleType.Har && (o = this.generateHarOhmurl(this.moduleName, s.getName(), t.pageSourceFile, true))), Object.assign(t, {
        ohmurl: o,
        bundleName: this.getBundleName(),
        moduleName: this.moduleName
      });
      const i = {
        ohmurl: o,
        name: t.name,
        pageSourceFile: a,
        buildFunction: t.buildFunction
      };
      this.routerMapAddPackageName(t, e), this.intermediatesRouterMapObjs.push(i);
    }), this.checkDependencyRouterMap(e, r), t.push(...r));
  }
  processRemoteDependencyRouterMap(e, t) {
    const a = dependency_manager_js_1.DependencyManager.getRemoteDependencyRouterMapObjList(e);
    if (!a) return;
    const s = dependency_manager_js_1.DependencyManager.getModuleObjByRemoteDependency(e),
      r = s.module.type;
    a.forEach(t => {
      var a;
      const o = t.pageSourceFile,
        i = path_1.resolve(e.getDependencyRootPath(), o);
      e.isByteCodeHarDependency() || this.pushCompileEntry(r, i);
      const n = s.module.name;
      let u;
      this.useNormalizedOHMUrl ? u = this.getOhmUrlWithNormalize(e, t.pageSourceFile) : (r === module_type_enum_js_1.ModuleType.Shared && (u = this.generateModuleOhmurl(n, t.pageSourceFile)), r === module_type_enum_js_1.ModuleType.Har && (u = this.generateHarOhmurl(this.moduleName, n, i, false, e.getDependencyName()))), Object.assign(t, {
        ohmurl: e.isBundleDependency() && null !== (a = t.ohmurl) && undefined !== a ? a : u,
        bundleName: this.getBundleName(),
        moduleName: this.moduleName
      });
      const l = {
        ohmurl: u,
        name: t.name,
        pageSourceFile: i,
        buildFunction: t.buildFunction
      };
      this.routerMapAddPackageName(t, e), this.intermediatesRouterMapObjs.push(l);
    }), this.checkDependencyRouterMap(e, a), t.push(...a);
  }
  routerMapAddPackageName(e, t) {
    e.packageName = e.packageName || t.getPackageName();
  }
  checkModuleRouterMapObj(e, t, a) {
    if (!e) return;
    if (this.targetService.isCustomizedHar()) return;
    const s = t.getName();
    e.forEach(e => {
      const r = a ? dependency_manager_js_1.DependencyManager.getDependencyModuleRouterMapJsonPath(t, this.moduleName2Target.get(s)) : this.targetService.getRouterMapJsonPath(t),
        o = this.getModulePageSourceFile(t, e);
      "" !== e.pageSourceFile && "" !== o && o && fs_extra_1.existsSync(o) || this._log.printErrorExit("PAGE_SOURCE_FILE_NOT_FOUND", [null != o ? o : "", s, r], [[s]]), common_const_js_1.ValidateRegExp.PAGE_SOURCE_FILE_REG_EXP.test(e.pageSourceFile) || this._log.printErrorExit("PAGE_SOURCE_FILE_FORMAT_INVALID", [s, r]);
    });
  }
  getModulePageSourceFile(e, t) {
    const a = t.pageSourceFile;
    if ("" !== a) return path_1.resolve(e.getModule().getNodeDir(), a);
  }
  checkDependencyRouterMap(e, t) {
    t && (this.targetService.isCustomizedHar() || t.forEach(t => {
      const a = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(e, this.projectModel);
      if (!a) return;
      const s = null == a ? undefined : a.getName(),
        r = dependency_manager_js_1.DependencyManager.getDependencyModuleRouterMapJsonPath(a, this.moduleName2Target.get(s)),
        o = t.pageSourceFile,
        i = path_1.resolve(e.getDependencyRootPath(), o);
      "" !== o && "" !== i && i && fs_extra_1.existsSync(i) || this._log.printErrorExit("PAGE_SOURCE_FILE_NOT_FOUND", ["" === o ? "" : i, s, null != r ? r : ""], [[s]]);
    }));
  }
  checkRouterMapObjName(e) {
    if (!e || e.length <= 1) return;
    const t = new Map();
    e.forEach(e => {
      const a = e.packageName;
      if (t.has(a)) {
        const s = t.get(a);
        if (!s) return;
        s.includes(e.name) || (s.push(e.name), t.set(a, s));
      } else t.set(a, [e.name]);
    });
    const a = new Map();
    if (t.forEach((e, s) => {
      const r = (0, array_util_js_1.findDuplicateElement)(e);
      t.forEach((t, a) => {
        if (a === s) return;
        const o = (0, array_util_js_1.getIntersection)(t, e);
        0 !== o.length && r.push(...o);
      }), 0 !== r.length && a.set(s, Array.from(new Set(r)));
    }), 0 !== a.size) {
      const e = a.get(undefined);
      e && (a.set(this.packageJsonObj.name, e), a.delete(undefined)), this._log.printErrorExit("DUPLICATE_ROUTERMAP_OBJECT_NAME_DETECTED", undefined, [[JSON.stringify(Object.fromEntries(a.entries()))]]);
    }
  }
  getBundleName() {
    var e;
    return null !== (e = this.targetData.getProduct().bundleName) && undefined !== e ? e : this.projectModel.getDefaultBundleName();
  }
  generateModuleOhmurl(e, t) {
    const a = (0, ohmurl_utils_js_1.removeSuffix)(t),
      s = a.includes("src/main/ets/") ? a.replace("src/main/", "") : a;
    return `@bundle:${this.getBundleName()}/${e}/${s}`;
  }
  generateHarOhmurl(e, t, a, s, r) {
    if (s) {
      const s = (0, ohmurl_utils_js_1.removeSuffix)(a),
        r = s.includes("src/main/ets/") ? s.replace("src/main/", "") : s;
      return `@bundle:${this.getBundleName()}/${e}@${t}/${r}`;
    }
    const o = path_1.parse(a).name,
      i = `${a.substring(0, a.lastIndexOf(path_1.sep) + 1)}${o}`,
      n = i.substring(i.indexOf(common_const_js_1.CommonConst.OH_MODULES)).replace(common_const_js_1.CommonConst.OH_MODULES, common_const_js_1.CommonConst.PKG_MODULES),
      u = n.indexOf(path_1.sep + (null == r ? undefined : r.replace("/", path_1.sep)) + path_1.sep),
      l = n.substring(0, u),
      d = l.replace(path_1.basename(l), common_const_js_1.CommonConst.PKG_MODULES) + n.substring(u),
      p = new RegExp("\\" + path_1.sep, "g");
    return `@package:${d.replace(p, "/")}`;
  }
  pushCompileEntry(e, t) {
    e !== module_type_enum_js_1.ModuleType.Shared && this.compileEntry.push(t);
  }
  initTaskDepends() {}
  getOhmUrlWithNormalize(e, t) {
    const a = e.getPackageName(),
      s = this.pathInfo.getIntermediatesPkgContextInfoPath(),
      r = json_util_js_1.JsonUtil.getJson5Obj(s);
    r || this._log.printErrorExit("PKG_CONTEXT_INFO_PATH_NOT_AVAILABLE", [s]);
    const o = r[a];
    return o || this._log.printErrorExit("PKG_NAME_NOT_EXIST", [a, s]), (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(o, t);
  }
  generateTempRouterMap(e, t) {
    fs_extra_1.ensureFileSync(this.intermediatesTempRouterMapFilePath), fs_extra_1.writeFileSync(this.intermediatesTempRouterMapFilePath, JSON.stringify({
      routerMap: e
    })), fs_extra_1.ensureFileSync(this.intermediatesToLoaderRouterMap), fs_extra_1.writeFileSync(this.intermediatesToLoaderRouterMap, JSON.stringify({
      routerMap: t
    }));
  }
  processHarDeduplicate(e, t) {
    this.moduleModel.isHapModule() ? this.addDuplicateRouterMap(e, t) : this.moduleModel.isHspModule() && this.collectDuplicateHarRouterMap(e, t);
  }
  addDuplicateRouterMap(e, t) {
    this.allHspDepTargets && this.allHspDepTargets.forEach((a, s) => {
      this.mergeRouterMap(a.getPathInfo().getRouterMapDuplicateInfoPath(), e, (e, t) => e.moduleNodeDir === t.moduleNodeDir, s), t && this.mergeRouterMap(a.getPathInfo().getLoaderRouterMapDuplicateInfoPath(), t, (e, t) => e.pageSourceFile === t.pageSourceFile, s);
    });
  }
  mergeRouterMap(e, t, a, s) {
    if (!fs_extra_1.existsSync(e)) return;
    const r = fs_extra_1.readJSONSync(e);
    r && ((0, deduplicate_util_js_1.mergeTwoArray)(r, t, a), this._log.debug(`HarDeduplicate add routerMap ${r} of hsp ${s}`));
  }
  collectDuplicateHarRouterMap(e, t) {
    if (!this.hspNeedDeduplicateHarInfo) return;
    const a = Object.keys(this.hspNeedDeduplicateHarInfo).map(e => path_1.normalize(e)),
      s = new Set(a),
      r = (0, deduplicate_util_js_1.deduplicateArray)(e, e => s.has(path_1.normalize(e.moduleNodeDir)));
    if (fs_extra_1.outputJSONSync(this.pathInfo.getRouterMapDuplicateInfoPath(), r), this._log.debug(`HarDeduplicate collected duplicate routerMap ${r} of hsp ${this.moduleName}`), t) {
      const e = (0, deduplicate_util_js_1.deduplicateArray)(t, e => undefined !== a.find(t => path_1.normalize(e.pageSourceFile).startsWith(t)));
      fs_extra_1.outputJSONSync(this.pathInfo.getLoaderRouterMapDuplicateInfoPath(), e), this._log.debug(`HarDeduplicate collected duplicate loader routerMap ${e} of hsp ${this.moduleName}`);
    }
  }
  processHarDeduplicateInputFiles(e) {
    this.allHspDepTargets && this.allHspDepTargets.forEach(t => {
      this.addInputFileIfExists(t.getPathInfo().getRouterMapDuplicateInfoPath(), e), this.addInputFileIfExists(t.getPathInfo().getLoaderRouterMapDuplicateInfoPath(), e);
    });
  }
  addInputFileIfExists(e, t) {
    fs_extra_1.existsSync(e) && t.addEntry(e);
  }
}
