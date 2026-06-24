/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-profile.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as fs from "fs-extra";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../const/common-const";
import * as virtual_machine_enum_js_1 from "../enum/virtual-machine-enum";
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as hsp_target_utils_js_1 from "../utils/hsp-target-utils";
import * as inject_util_js_1 from "../utils/inject-util";
import * as integrated_hsp_utils_js_1 from "../utils/integrated-hsp-utils";
import * as json_util_js_1 from "../utils/json-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ohmurl_utils_js_1 from "../utils/ohmurl-utils";
import * as task_names_js_1 from "./common/task-names";
import * as deduplicate_util_js_1 from "./har-deduplicate/deduplicate-util";
import * as merge_profile_js_1 from "./merge-profile";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
var __decorate = this && this.__decorate || function (e, t, i, s) {
  var o,
    l = arguments.length,
    n = l < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, i) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(e, t, i, s);else for (var r = e.length - 1; r >= 0; r--) (o = e[r]) && (n = (l < 3 ? o(n) : l > 3 ? o(t, i, n) : o(t, i)) || n);
  return l > 3 && n && Object.defineProperty(t, i, n), n;
};
var Task = task_names_js_1.TaskNames.Task;
const INPUT_KEY = {
  arkEnable: "arkEnable",
  compileMode: "compileMode",
  deviceTypes: "deviceTypes",
  dependency: "dependency",
  harExcludeHSPDependencies: "harExcludeHSPDependencies"
};
export class ProcessProfile extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, Task.PROCESS_PROFILE), this._log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-process-profile"), this._dependencies = [], this.harUIAbilities = [], this.harExtensionAbilities = [], this.duplicateAbilities = {
      [common_const_js_1.AbilityConst.UI_ABILITY]: [],
      [common_const_js_1.AbilityConst.EXTENSION_ABILITY]: []
    }, this._arkEnable = this.service.isArkModule(), this._moduleTargetData = this.targetService.getTargetData(), this._deviceTypes = this._moduleTargetData.getTargetDeviceType(), this._pathInfo = this._moduleTargetData.getPathInfo(), this._intermediatesMergeProfile = this._pathInfo.getIntermediatesMergeProfile(), this._processedModuleJson = this._pathInfo.getIntermediatesProcessProfile(), this.fakeUIAbilityPath = path_1.resolve(this.pathInfo.getModuleBuildPath(), common_const_js_1.PreviewerConst.ABILITY_ETS_PATH), this.mergeHspLibs().forEach(e => this._dependencies.push(e)), this.initHarDeduplicateData();
  }
  initTaskDepends() {
    this.declareDepends(merge_profile_js_1.MergeProfile.name);
  }
  initHarDeduplicateData() {
    if (this.targetService.shouldDoDeduplicateTask()) if (this.moduleModel.isHapModule()) {
      const e = (0, collect_duplicate_har_dependencies_js_1.getMergedHapAllHspDependencies)();
      this.allHspDepTargets = hsp_target_utils_js_1.HspTargetUtils.calHspTargets(this.targetService, e);
    } else if (this.moduleModel.isHspModule()) {
      const e = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName);
      e && (this.hspNeedDeduplicateHarRootPathSet = new Set(Object.keys(e)));
    }
  }
  shouldCollectDependenciesAbilities() {
    return !inject_util_js_1.InjectUtil.isLocalTest() && (this.moduleModel.isHapModule() || this.moduleModel.isHspModule() || inject_util_js_1.InjectUtil.isOhosTest() || inject_util_js_1.InjectUtil.isPreviewProcess());
  }
  get allowToAddFakeUIAbility() {
    return inject_util_js_1.InjectUtil.isPreviewProcess() && !this.moduleModel.isHarModule();
  }
  get isFakeUIAbilityExists() {
    return this.allowToAddFakeUIAbility && fs_extra_1.existsSync(this.fakeUIAbilityPath);
  }
  addFakeUIAbility(e) {
    if (!this.allowToAddFakeUIAbility) return void this._log.debug("No need to add fake ui ability because of " + (this.moduleModel.isHarModule() ? "in HAR module." : "not in preview process."));
    if (!this.isFakeUIAbilityExists) return void this._log.debug("No need to add fake ui ability because of abilityPath does not exist.");
    e.module.abilities = e.module.abilities || [];
    e.module.abilities.push({
      name: common_const_js_1.PreviewerConst.FAKE_UI_ABILITY,
      srcEntry: common_const_js_1.PreviewerConst.ENTRY_ABILITY_PATH,
      startWindowIcon: "startIcon",
      startWindowBackground: "#FFFFFF"
    });
  }
  async beforeAlwaysAction() {
    if (await super.beforeAlwaysAction(), this.shouldCollectDependenciesAbilities()) {
      const e = this.collectHarAbilities();
      this.harUIAbilities = e[common_const_js_1.AbilityConst.UI_ABILITY], this.harExtensionAbilities = e[common_const_js_1.AbilityConst.EXTENSION_ABILITY];
    }
    this.validateExtensionAbilities();
  }
  validateExtensionAbilities() {
    const e = json_util_js_1.JsonUtil.getJson5Obj(this._intermediatesMergeProfile).module.extensionAbilities || [];
    if (e.push(...this.harExtensionAbilities), !e.length) return;
    const t = this.findDuplicatedAbilityNames(e);
    t.length && this._log.warn(`Duplicate 'Extension-Abilities' object names detected: ${JSON.stringify(t)}. \n    Make sure the names are unique across the current module and the modules on which it depends.`);
  }
  doTaskAction() {
    const e = json_util_js_1.JsonUtil.getJson5Obj(this._intermediatesMergeProfile),
      t = this.projectModel.getCompatibleApiMetaByProduct(this.targetData.getProduct().name).version;
    e.module.virtualMachine = this._arkEnable ? `${virtual_machine_enum_js_1.VirtualMachine.ARK}${this.sdkInfo.getArkVersion(t, this.compatibleSdkVersionStage)}` : virtual_machine_enum_js_1.VirtualMachine.DEFAULT, e.module.compileMode = this.targetCompileMode, e.module.deviceTypes = this._deviceTypes, this.addModuleDependencies(e);
    const i = this.pathInfo.getIntermediatesArkModuleJsonPath();
    fs.ensureFileSync(i), fs.writeFileSync(i, JSON.stringify(e)), this.moduleModel.isFormExtensionModule(this.targetName) && this.processFormExtensionAbilities(e), this.shouldCollectDependenciesAbilities() && (this.processDependencyHarAbility(e), this.processHarDeduplicate(e)), this.addFakeUIAbility(e), fs.outputJSONSync(this._processedModuleJson, e, {
      spaces: "\t"
    });
  }
  declareInputs() {
    const e = new Map();
    return e.set(INPUT_KEY.arkEnable, this._arkEnable), e.set(INPUT_KEY.compileMode, this.targetCompileMode), e.set(INPUT_KEY.deviceTypes, this._deviceTypes), e.set(INPUT_KEY.dependency, JSON.stringify(this._dependencies)), e.set(INPUT_KEY.harExcludeHSPDependencies, hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.HAR_EXCLUDE_HSP_DEPENDENCIES)), e.set("compatibleSdkVersionStage", this.compatibleSdkVersionStage || ""), e;
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet().addEntry(this._intermediatesMergeProfile);
    return this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule() && this.processHarDeduplicateInputFiles(e), e;
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet().addEntry(this._processedModuleJson);
    return this.hasDuplicatedHarAbilities() && e.addEntry(this.pathInfo.getAbilityDuplicateInfoPath()), e;
  }
  mergeHspLibs() {
    const e = this.service.getHspDependencyPaths(),
      t = this.service.getHspModuleDependencyPathsWithOutDynamic(),
      {
        versionCode: i
      } = (0, integrated_hsp_utils_js_1.getAppInfoByProject)(this.projectModel);
    return [...(0, wdk_1.difference)(e, t).map(e => {
      const t = path_1.resolve(e, "src", "main", "module.json"),
        i = path_1.resolve(e, "src", "main", "module.json5");
      return fs_extra_1.existsSync(t) ? t : i;
    }), ...t.map(e => path_1.resolve(e, "src", "main", "module.json5"))].map(e => {
      let t;
      return t = e.endsWith("module.json") ? fs_extra_1.readJsonSync(e) : hvigor_1.Json5Reader.getJson5Obj(e), t;
    }).map(e => {
      var t, s, o, l, n;
      const r = null === (t = null == e ? undefined : e.app) || undefined === t ? undefined : t.bundleType,
        a = {
          moduleName: e.module.name,
          versionCode: (null === (s = null == e ? undefined : e.app) || undefined === s ? undefined : s.bundleName) && null !== (l = null === (o = null == e ? undefined : e.app) || undefined === o ? undefined : o.versionCode) && undefined !== l ? l : i
        };
      return r === common_const_js_1.BundleType.SHARED && (a.bundleName = null === (n = null == e ? undefined : e.app) || undefined === n ? undefined : n.bundleName), a;
    });
  }
  collectHarAbilities() {
    const e = {
      [common_const_js_1.AbilityConst.UI_ABILITY]: [],
      [common_const_js_1.AbilityConst.EXTENSION_ABILITY]: []
    };
    return this.service.getHarDependencies().forEach(t => {
      var i;
      if (inject_util_js_1.InjectUtil.isPreviewProcess() && t.isByteCodeHarDependency()) return;
      const s = t.getModuleJsonObj();
      if (s) for (const o of [common_const_js_1.AbilityConst.UI_ABILITY, common_const_js_1.AbilityConst.EXTENSION_ABILITY]) {
        const l = null === (i = s.module) || undefined === i ? undefined : i[o];
        (null == l ? undefined : l.length) && l.forEach(i => {
          const s = (0, wdk_1.cloneDeep)(i);
          s.srcEntry && (Object.assign(s, {
            srcEntry: (0, ohmurl_utils_js_1.generateOhmurlForSrcEntry)(this.targetService, this.moduleName, t, s.srcEntry),
            moduleName: t.getDependencyName()
          }), e[o] && (this.shouldExcludeDuplicateHarDependency(t) ? (this.duplicateAbilities[o].push(s), this._log.debug(`HarDeduplicate exclude har ${t.getPackageName()} of hsp: ${this.moduleName}`)) : e[o].push(s)));
        });
      }
    }), e;
  }
  addHarAbilitiesToModuleJson(e, t, i) {
    var s;
    i.length && ((s = e.module)[t] || (s[t] = []), e.module[t].push(...i));
  }
  processDependencyHarAbility(e) {
    e && (this.addHarAbilitiesToModuleJson(e, common_const_js_1.AbilityConst.UI_ABILITY, this.harUIAbilities), this.addHarAbilitiesToModuleJson(e, common_const_js_1.AbilityConst.EXTENSION_ABILITY, this.harExtensionAbilities), this.checkDuplicateAbility(e.module[common_const_js_1.AbilityConst.UI_ABILITY]));
  }
  findDuplicatedAbilityNames(e) {
    if (!(null == e ? undefined : e.length)) return [];
    const t = new Map();
    return e.forEach(e => {
      var i, s;
      const o = null !== (i = e.moduleName) && undefined !== i ? i : this.moduleName;
      t.has(e.name) ? null === (s = t.get(e.name)) || undefined === s || s.push(o) : t.set(e.name, [o]), e.moduleName && delete e.moduleName;
    }), Array.from(t.entries()).filter(([e, t]) => t.length > 1);
  }
  checkDuplicateAbility(e) {
    if (!(null == e ? undefined : e.length)) return;
    const t = this.findDuplicatedAbilityNames(e);
    0 !== t.length && this._log.printErrorExit("DUPLICATE_MODULE_ABLITIES_OBJECT_NAMES_DETECTED", undefined, [[JSON.stringify([...t])]]);
  }
  addModuleDependencies(e) {
    this.moduleModel.isHarModule() && true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.HAR_EXCLUDE_HSP_DEPENDENCIES) || (e.module.dependencies = this._dependencies);
  }
  processFormExtensionAbilities(e) {
    e.module.extensionAbilities && (e.module.extensionAbilities = e.module.extensionAbilities.filter(e => "form" !== e.type));
  }
  shouldExcludeDuplicateHarDependency(e) {
    return !!this.targetService.shouldDoDeduplicateTask() && !!this.moduleModel.isHspModule() && !!this.hspNeedDeduplicateHarRootPathSet && this.hspNeedDeduplicateHarRootPathSet.has(e.getDependencyRootPath());
  }
  hasDuplicatedHarAbilities() {
    return this.duplicateAbilities[common_const_js_1.AbilityConst.UI_ABILITY].length > 0 || this.duplicateAbilities[common_const_js_1.AbilityConst.EXTENSION_ABILITY].length > 0;
  }
  processHarDeduplicate(e) {
    this.targetService.shouldDoDeduplicateTask() && (this.moduleModel.isHapModule() ? this.addDuplicateAbilities(e) : this.moduleModel.isHspModule() && this.writeDuplicateHarAbilitiesToIntermediateFile());
  }
  addDuplicateAbilities(e) {
    this.allHspDepTargets && this.allHspDepTargets.forEach((t, i) => {
      const s = t.getPathInfo().getAbilityDuplicateInfoPath();
      if (!fs_extra_1.existsSync(s)) return;
      const o = fs_extra_1.readJSONSync(s);
      o && (this.mergeDuplicateAbilities(e, o, common_const_js_1.AbilityConst.UI_ABILITY, i), this.mergeDuplicateAbilities(e, o, common_const_js_1.AbilityConst.EXTENSION_ABILITY, i));
    });
  }
  mergeDuplicateAbilities(e, t, i, s) {
    !t[i] || t[i].length <= 0 || (e.module[i] = e.module[i] || [], (0, deduplicate_util_js_1.mergeTwoArray)(t[i], e.module[i], (e, t) => e.name === t.name, e => {
      this._log.debug(`HarDeduplicate add ability ${e} of hsp ${s}`);
    }));
  }
  writeDuplicateHarAbilitiesToIntermediateFile() {
    this.hasDuplicatedHarAbilities() && fs.outputJSONSync(this.pathInfo.getAbilityDuplicateInfoPath(), this.duplicateAbilities);
  }
  processHarDeduplicateInputFiles(e) {
    this.allHspDepTargets && this.allHspDepTargets.forEach(t => {
      const i = t.getPathInfo().getAbilityDuplicateInfoPath();
      fs_extra_1.existsSync(i) && e.addEntry(i);
    });
  }
}
__decorate([(0, hvigor_1.Input)()], ProcessProfile.prototype, "isFakeUIAbilityExists", null);
