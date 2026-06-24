/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/process-har-artifacts.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as glob_1 from "glob";
import * as common_util_js_1 from "../../common/common-util";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as dependency_interface_js_1 from "../../project/dependency/core/dependency-interface";
import * as dependency_manager_js_1 from "../../project/dependency/dependency-manager";
import * as build_profile_utils_js_1 from "../../utils/build-profile-utils";
import * as copy_resources_util_js_1 from "../../utils/copy-resources-util";
import * as dependency_util_js_1 from "../../utils/dependency-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as har_target_util_js_1 from "../../utils/har-target-util";
import * as json_util_js_1 from "../../utils/json-util";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as oh_exports_util_js_1 from "../../utils/oh-exports-util";
import * as ohmurl_utils_js_1 from "../../utils/ohmurl-utils";
import * as parameter_utils_js_1 from "../../utils/parameter-utils";
import * as validate_systemHsp_js_1 from "../../utils/validate/validate-systemHsp";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as abstract_process_har_artifacts_js_1 from "../abstract/abstract-process-har-artifacts";
import * as cache_native_libs_js_1 from "../cache-native-libs";
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as process_obfuscation_files_js_1 from "./process-obfuscation-files";
var __decorate = this && this.__decorate || function (e, t, s, i) {
  var r,
    o = arguments.length,
    a = o < 3 ? t : null === i ? i = Object.getOwnPropertyDescriptor(t, s) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, t, s, i);else for (var n = e.length - 1; n >= 0; n--) (r = e[n]) && (a = (o < 3 ? r(a) : o > 3 ? r(t, s, a) : r(t, s)) || a);
  return o > 3 && a && Object.defineProperty(t, s, a), a;
};
var Task = task_names_js_1.TaskNames.Task;
export class ProcessHarArtifacts extends abstract_process_har_artifacts_js_1.AbstractProcessHarArtifacts {
  constructor(e) {
    super(e, Task.PROCESS_HAR_ARTIFACTS), this.logger = ohos_logger_js_1.OhosLogger.getLogger(ProcessHarArtifacts.name), this.abilities = common_const_js_1.AbilityConst.UI_ABILITY, this.extensionAbilities = common_const_js_1.AbilityConst.EXTENSION_ABILITY, this.compiledPackageNames = [], this.npmImportees = [], this.affectsCompilationDependencyKeys = [], this.globalAffectsCompilationDependencyKeys = [], this.arkTsCompiledOutputDir = path_1.resolve(this.pathInfo.getInterMediatesLoaderOutPath()), this.compiledAbcOutputDir = path_1.resolve(this.arkTsCompiledOutputDir, code_type_enum_js_1.CodeType.ETS), this.declaredFilesOutputDir = this.pathInfo.getIntermediatesEtsTgzPath(this.moduleName);
    const t = this.projectModel.getAppRes();
    this.appOptObj = t.getAppResOpt();
    const s = e.getTargetData().getModuleSourceSetModel().getModuleTargetRes();
    this.targetModuleOptObj = s.getModuleJsonOpt(), this.targetJsonPath = s.getJsonPath(), this.pacJsonFilePath = this.pathInfo.getIntermediatesPacJsonPath(), this.ohPkgMetadataResDir = [], this.nonEntryImportees = [];
  }
  get isBundledDependencies() {
    return this.targetService.isBundledDependencies();
  }
  get isNativeStripped() {
    var e, t, s, i;
    return null === (i = null === (s = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.nativeLib) || undefined === t ? undefined : t.debugSymbol) || undefined === s ? undefined : s.strip) || undefined === i || i;
  }
  get isByteCodeHarOptimize() {
    return inject_util_js_1.InjectUtil.isByteCodeHarOptimize();
  }
  get shouldPackSourceMaps() {
    var e, t;
    if (!this.targetService.isByteCodeHar()) return false;
    const s = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.packSourceMap;
    return undefined !== s ? s : this.targetService.isDebug();
  }
  get insightIntentJsonPath() {
    return path_1.resolve(this.pathInfo.getModuleSrcMainResourceBaseProfilePath(), build_directory_const_js_1.BuildArtifactConst.INSIGHT_INTENT_JSON);
  }
  get customTypes() {
    return this.targetService.getCustomTypes();
  }
  get preloadSoFilePath() {
    return this.pathInfo.getPreloadSoFilePath();
  }
  get isDependenciesTypesEnable() {
    return inject_util_js_1.InjectUtil.isDependenciesTypesEnable();
  }
  declareInputs() {
    var e, t, s, i, r, o;
    const a = (0, wdk_1.cloneDeep)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt),
      n = super.declareInputs().set("bundleType", null !== (s = null !== (t = null == a ? undefined : a.bundleType) && undefined !== t ? t : (0, find_target_product_js_1.findTargetProduct)(this.service.getProjectModel()).bundleType) && undefined !== s ? s : this.projectModel.getBundleType()).set("isByteCodeHar", this.targetService.isByteCodeHar()).set("harLocalDependencyCheck", !!(null === (i = this.targetService.getBuildOption().strictMode) || undefined === i ? undefined : i.harLocalDependencyCheck)).set("isBundledDependencies", this.isBundledDependencies).set("nonEntryImportees", this.nonEntryImportees).set("isDebug", this.targetService.isDebug()).set("isNativeStripped", this.isNativeStripped);
    n.set("appStartupFileName", null !== (r = this.appStartupFileName) && undefined !== r ? r : ""), n.set("isOpenSource", this.targetService.isOpenSource());
    const l = null === (o = this.targetService.getTargetData().getTargetOpt().resource) || undefined === o ? undefined : o.directories;
    return l && n.set("targetDirectories", l), n;
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return fs_extra_1.existsSync(this.pathInfo.getIntermediatesPkgContextInfoPath()) && e.addEntry(this.pathInfo.getIntermediatesPkgContextInfoPath()), this.appStartupPath && fs_extra_1.existsSync(this.appStartupPath) && e.addEntry(this.appStartupPath), fs_extra_1.existsSync(this.intermediateTempStartupFilePath) && e.addEntry(this.intermediateTempStartupFilePath), fs_extra_1.existsSync(this.pacJsonFilePath) && e.addEntry(this.pacJsonFilePath), e;
  }
  declareOutputFiles() {
    return super.declareOutputFiles().addEntry(path_1.resolve(this.taskTmpDir, common_const_js_1.CommonConst.PAC_JSON));
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.collectNonEntryImportees(), this.collectCompiledAndNpmImportees(), this.isByteCodeHarOptimize && (this.processDirectImportees(), this.processAllImportees()), this.warnWhenPackSourceMapsInReleaseMode();
  }
  warnWhenPackSourceMapsInReleaseMode() {
    !this.targetService.isDebug() && this.shouldPackSourceMaps && this.logger.warn("The sourceMaps.map file will be packed into the HAR in release mode. This can potentially lead to code asset leakage within the HAR.\n               To prevent this, set packSourceMap to false.");
  }
  getImporteesCachePath(e) {
    return super.getImporteesCachePath(e, "HarCompileArkTS");
  }
  initTaskDepends() {
    this.declareDependsList(cache_native_libs_js_1.CacheNativeLibs.name, compile_resource_js_1.CompileResource.name), this.addSpecialDepends();
  }
  addSpecialDepends() {
    if (this._harExtendInfo.isObfuscatedHar()) {
      const e = this.sdkInfo.hasRollUpPluginInEtsLoader;
      this.declareDepends(e ? "HarCompileArkTS" : "HarBuildArkTS");
    } else this.declareDepends(common_const_js_1.ArktSTaskConst.LINT_ARKTS);
    this.declareDepends(this.moduleModel.isOhpmProject() ? Task.PROCESS_OH_PACKAGE_JSON.name : Task.PROCESS_PACKAGE_JSON.name), this.declareDepends(process_obfuscation_files_js_1.ProcessObfuscationFiles.name);
  }
  copyCompiledSourceFileToTempDir() {
    if (this.targetService.isByteCodeHar()) {
      if (fs_extra_1.existsSync(this.compiledAbcOutputDir) && fs_extra_1.copySync(this.compiledAbcOutputDir, path_1.join(this.taskTmpDir, path_1.basename(this.compiledAbcOutputDir))), fs_extra_1.existsSync(this.declaredFilesOutputDir)) {
        fs_extra_1.copySync(this.declaredFilesOutputDir, this.taskTmpDir);
        const e = path_1.resolve(this.taskTmpDir, this.pathInfo.getBuildRoot());
        fs_extra_1.existsSync(e) && fs_extra_1.removeSync(e);
      }
    } else fs_extra_1.existsSync(this.arkTsCompiledOutputDir) && fs_extra_1.copySync(this.arkTsCompiledOutputDir, this.taskTmpDir);
  }
  copyDeclareFileToTempDir() {
    var e, t;
    const s = this.module.getNodeDir(),
      i = path_1.basename(this.packageManagerPath),
      r = path_1.resolve(this.taskTmpDir, i);
    if (!fs_extra_1.existsSync(r)) return;
    const o = json_util_js_1.JsonUtil.getJson5Obj(r),
      a = o.types,
      n = [],
      l = [];
    null === (t = null === (e = o.metadata) || undefined === e ? undefined : e.sourceRoots) || undefined === t || t.forEach(e => {
      const t = e.startsWith("./") ? e.slice(2) : e;
      n.push(`${t}/**/*.{d.ets,d.ts}`), l.push(`${t}/resource/**`);
    });
    (0, glob_1.globSync)([a, ...n], {
      cwd: s,
      nodir: true,
      dot: true,
      ignore: l
    }).filter(e => e.endsWith(".d.ets") || e.endsWith(".d.ts")).forEach(e => {
      const t = path_1.resolve(s, e),
        i = path_1.resolve(this.taskTmpDir, e);
      fs_extra_1.existsSync(i) || fs_extra_1.copySync(t, i);
    });
  }
  processBundleDepRes() {
    var e;
    if (!this.targetService.isBundledDependencies()) return;
    const t = `${build_directory_const_js_1.BuildDirConst.SRC}${path_1.sep}${build_directory_const_js_1.BuildDirConst.MAIN}${path_1.sep}${build_directory_const_js_1.BuildDirConst.RESOURCES}`,
      s = null === (e = this.targetService.getTargetData().getTargetOpt().resource) || undefined === e ? undefined : e.directories;
    (null == s ? undefined : s.length) ? this.ohPkgMetadataResDir.push(...s) : this.ohPkgMetadataResDir.push(t);
    const i = har_target_util_js_1.HarTargetUtil.getDep2ConfigMap(this.targetService);
    i && i.dep2Resources.forEach((e, t) => {
      this.copyBundledDepRes(e, t);
    });
  }
  copyBundledDepRes(e, t) {
    const s = json_util_js_1.JsonUtil.getJson5Obj(path_1.resolve(t.getDependencyRootPath(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5));
    e.forEach(e => {
      this.ohPkgMetadataResDir.push(`${e}-${s.name}`);
      const i = path_1.resolve(t.getDependencyRootPath(), e);
      if (fs_extra_1.existsSync(i)) {
        const t = path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, `${path_1.basename(e)}-${s.name}`);
        fs_extra_1.mkdirsSync(t), file_util_js_1.FileUtil.copyDir(i, t);
      }
    });
  }
  copyOtherGenerateFiles() {
    if (this._harExtendInfo.isObfuscatedHar()) fs_extra_1.readdirSync(this.generateDir).forEach(e => {
      const t = path_1.resolve(this.generateDir, e);
      fs_extra_1.copySync(t, path_1.resolve(this.taskTmpDir, e));
    });else try {
      fs_extra_1.copySync(oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(this.generateDir, this.isOhpmProject ? common_const_js_1.CommonConst.OH_PACKAGE_JSON5 : common_const_js_1.CommonConst.PACKAGE_JSON)), oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(this.taskTmpDir, this.isOhpmProject ? common_const_js_1.CommonConst.OH_PACKAGE_JSON5 : common_const_js_1.CommonConst.PACKAGE_JSON)));
    } catch (e) {
      this.logger.warn(e.message);
    }
  }
  copyIntermediatePacToTempDir() {
    fs_extra_1.existsSync(this.pacJsonFilePath) && fs_extra_1.copySync(this.pacJsonFilePath, path_1.resolve(this.taskTmpDir, common_const_js_1.CommonConst.PAC_JSON));
  }
  processHarRouterMap() {
    const e = this.moduleModel.getJsonPathByTargetName(this.targetName),
      t = res_model_loader_js_1.resModelLoader.getModuleJson(e).module.routerMap,
      s = (0, common_util_js_1.parsingProfileName)(t);
    if (!s) return;
    const i = path_1.resolve(this.targetData.getPathInfo().getIntermediatesResProfilePath(), `${s}.json`),
      r = path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR, `${s}.json`),
      o = json_util_js_1.JsonUtil.getJson5Obj(i);
    this.targetService.isOpenSource() || o.routerMap.forEach(e => {
      const t = this.getObfuscatedPageSourceFile(this.moduleModel, e, this.getObfNameCacheObj());
      t && Object.assign(e, {
        pageSourceFile: t
      });
    }), o.routerMap.map(e => delete e.originalSuffix), fs_extra_1.writeFileSync(r, JSON.stringify(o));
  }
  addPreloadSystemSoJson() {
    if (!this.targetService.isByteCodeHar() || !this.isPreloadSystemSoJsonEffective) return;
    const e = path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildDirConst.META_INF);
    fs_extra_1.ensureDirSync(e), fs_extra_1.copyFileSync(this.preloadSoFilePath, path_1.resolve(e, path_1.basename(this.preloadSoFilePath)));
  }
  processHarResStartupConfig() {
    var e, t, s;
    const i = null !== (e = this.appStartupFileName) && undefined !== e ? e : "startup_config",
      r = path_1.resolve(this.targetData.getPathInfo().getIntermediatesResProfilePath(), `${i}.json`);
    if (!fs_extra_1.existsSync(r)) return;
    const o = json_util_js_1.JsonUtil.getJson5Obj(r);
    if (!o) return;
    const a = this.getTargetResPath(i);
    if (!a) return;
    const n = path_1.resolve(this.taskTmpDir, a);
    null === (t = o.startupTasks) || undefined === t || t.forEach(e => {
      var t;
      delete e.moduleName, delete e.srcEntryAbsolutePath, delete e.startupConfigPath, this.targetService.isOpenSource() || (e.srcEntry = null !== (t = this.getObfuscationSrcEntryPath(this.moduleModel, e.srcEntry, this.getObfNameCacheObj())) && undefined !== t ? t : e.srcEntry);
    }), null === (s = o.appPreloadHintStartupTasks) || undefined === s || s.forEach(e => {
      delete e.moduleName, delete e.startupConfigPath, delete e.isRemoteByteCodeHar;
    }), fs_extra_1.writeFileSync(n, JSON.stringify(o));
  }
  getTargetResPath(e) {
    var t, s;
    const i = null === (s = null === (t = this.targetService.getTargetData().getTargetOpt()) || undefined === t ? undefined : t.resource) || undefined === s ? undefined : s.directories;
    if (!(null == i ? undefined : i.length)) {
      const t = path_1.join(build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR, `${e}.json`),
        s = file_util_js_1.FileUtil.convertToAbsolutePath(t, this.targetService.getTargetData().getPathInfo().getModulePath());
      return fs_extra_1.existsSync(s) ? t : undefined;
    }
    for (let t = 0; t < i.length; t++) {
      const s = i[t],
        r = path_1.join(s, common_const_js_1.CommonConst.BASE_PROFILE, `${e}.json`),
        o = file_util_js_1.FileUtil.convertToAbsolutePath(r, this.targetService.getTargetData().getPathInfo().getModulePath());
      if (fs_extra_1.existsSync(o)) return r;
    }
  }
  getObfuscationSrcEntryPath(e, t, s) {
    let i;
    const r = this.hasUseTsHarField() ? ".ts" : ".js",
      o = file_util_js_1.FileUtil.getFileSuffix(t);
    common_const_js_1.ValidateRegExp.DYNAMIC_IMPORT_FILE_SUFFIX_REG_EXP.test(o) && (i = `${t.substring(0, t.lastIndexOf("."))}${r}`);
    const a = e.getName(),
      n = path_1.join(a, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, t),
      l = file_util_js_1.FileUtil.normalizePathSeparator(n);
    Object.prototype.hasOwnProperty.call(s, l) && (i = s[l].obfName);
    const c = path_1.relative(file_util_js_1.FileUtil.normalizePathSeparator(path_1.join(a, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN)), null != i ? i : l);
    return c.replace(path_1.extname(c), r);
  }
  processDeclarationEntry(e) {
    var t, s, i, r, o, a, n, l;
    if (!this.targetService.isByteCodeHar()) return void this.logger.debug("Since current module is not byte code har, just return.");
    const c = (null === (s = null === (t = e.metadata) || undefined === t ? undefined : t.runtimeOnly) || undefined === s ? undefined : s.sources) || [],
      d = (null === (i = e.metadata) || undefined === i ? undefined : i.workers) || [];
    null === (r = e.metadata) || undefined === r || delete r.workers, null === (a = null === (o = e.metadata) || undefined === o ? undefined : o.runtimeOnly) || undefined === a || delete a.sources;
    const _ = this.pathInfo.getIntermediatesPkgContextInfoPath(),
      u = json_util_js_1.JsonUtil.getJson5Obj(_),
      h = null == u ? undefined : u[this.packageJsonObj.name];
    if (!h) return void this.logger.debug(`Can not find package context information with package name: ${this.packageJsonObj.name}`);
    const p = [];
    this.isBundledDependencies && (this.collectConfigOhmurl(null === (n = har_target_util_js_1.HarTargetUtil.getDep2ConfigMap(this.targetService)) || undefined === n ? undefined : n.dep2Workers, u, p), this.collectRuntimeOnlyOhmurl(u, p), this.collectConfigOhmurl(null === (l = har_target_util_js_1.HarTargetUtil.getDep2ConfigMap(this.targetService)) || undefined === l ? undefined : l.dep2RouterMapObjPageSourceFiles, u, p)), this.collectByteCodeHarAbilityOhmurl(u, p), this.collectByteCodeHarStartupTasks(p), this.collectByteCodeHarInsightIntentSrcEntry(u, p);
    const g = [...d, ...c];
    g.length && g.forEach(e => {
      p.push((0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(h, e.replace(/^\.\//, "")));
    }), this.targetService.isCustomizedHar() && this.mergeDeclarationEntry(e, p), e.metadata = {
      ...e.metadata,
      declarationEntry: p
    };
  }
  processDirectImportees() {
    if (!this.targetService.isByteCodeHar()) return;
    const e = this.getImporteesCachePath(hvigor_arkts_compose_1.DIRECT_IMPORTEE_CACHE_FILE_NAME),
      {
        compileDependencyKeys: t,
        globalCompileDependencyKeys: s
      } = this.collectDirectAffectsCompilationDependencyKeys(e);
    [t, s].forEach(e => {
      e.sort((e, t) => e > t ? 1 : -1);
    }), this.affectsCompilationDependencyKeys = t, this.globalAffectsCompilationDependencyKeys = s;
  }
  getDependencyRuntimePackages(e, t) {
    var s, i, r, o;
    if (!e.isLocal()) {
      return (null === (i = null === (s = e.getPackageJsonObj().metadata) || undefined === s ? undefined : s.runtimeOnly) || undefined === i ? undefined : i.packages) || [];
    }
    const a = (0, dependency_util_js_1.getLocalDependencyBuildOptions)(e, t);
    return (null === (o = null === (r = null == a ? undefined : a.arkOptions) || undefined === r ? undefined : r.runtimeOnly) || undefined === o ? undefined : o.packages) || [];
  }
  processAllImportees() {
    var e, t, s;
    if (!this.isBundledDependencies) return;
    const i = this.service.getAffectsCompilationDependencies(),
      r = new Set((null === (s = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.runtimeOnly) || undefined === s ? undefined : s.packages) || []),
      o = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService),
      a = this.getImporteesCachePath(hvigor_arkts_compose_1.COMPILED_IMPORTEE_CACHE_FILE_NAME);
    this.processImportees(a, i, (e, t, s) => {
      r.add(s.getDependencyName()), s.isHarDependency() && !s.isByteCodeHarDependency() && this.getDependencyRuntimePackages(s, o).forEach(e => r.add(e));
    });
    const n = Array.from(r);
    n.sort((e, t) => e > t ? 1 : -1), this.affectsCompilationDependencyKeys = n;
  }
  collectNonEntryImportees() {
    if (!this.targetService.isByteCodeHar() || this.isBundledDependencies) return;
    const e = new Set(),
      t = this.isByteCodeHarOptimize ? this.service.getAllDirectDependencies() : this.service.getDirectDependencies(),
      s = this.getImporteesCachePath(hvigor_arkts_compose_1.NON_ENTRY_IMPORTEE_CACHE_FILE_NAME);
    this.processImportees(s, t, (t, s) => {
      s === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER && e.add(t);
    });
    const i = Array.from(e);
    i.sort((e, t) => e > t ? 1 : -1), this.nonEntryImportees = i;
  }
  collectCompiledAndNpmImportees() {
    if (!this.isBundledDependencies) return;
    const e = this.isByteCodeHarOptimize ? this.service.getAffectsCompilationDependencies() : this.service.getAllDependencies(),
      t = new Set(),
      s = new Set(),
      i = this.getImporteesCachePath(hvigor_arkts_compose_1.COMPILED_IMPORTEE_CACHE_FILE_NAME);
    this.processImportees(i, e, (e, i, r) => {
      i !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER ? i !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR || r.isByteCodeHarDependency() || t.add(e) : e !== r.getDependencyName() && s.add(e);
    });
    const r = Array.from(t),
      o = Array.from(s);
    r.sort((e, t) => e > t ? 1 : -1), o.sort((e, t) => e > t ? 1 : -1), this.compiledPackageNames = r, this.npmImportees = o;
  }
  hasSoFile() {
    const e = [this.processedLibs];
    for (; e.length;) {
      const t = e.pop();
      for (const s of fs_extra_1.readdirSync(t)) {
        const i = path_1.resolve(t, s),
          r = fs_extra_1.lstatSync(i);
        if (r.isFile() && !s.endsWith(".a")) return true;
        r.isDirectory() && e.push(i);
      }
    }
    return false;
  }
  copyPkgTypesFilePath(e) {
    const t = e.types;
    if (t) {
      const e = path_1.resolve(this.moduleModel.getModule().getNodeDir(), t),
        s = path_1.resolve(this.taskTmpDir, t);
      fs_extra_1.ensureDirSync(path_1.dirname(s)), fs_extra_1.existsSync(e) && fs_extra_1.copyFileSync(e, s);
    }
  }
  forEachDependencies(e, t) {
    e.forEach(e => {
      const s = e.getDependencyType();
      s !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR && s !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER || t(e.getDependencyName(), e.getDependencyVersion());
    });
  }
  setDependencyPkgVersion(e) {
    var t, s;
    const i = {
      parameterFilePath: this.service.getProjectModel().getParameterFileAbsolutePath(),
      ohPackageFilePath: this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath()
    };
    if (this.isBundledDependencies) return (t = e.metadata).dependencyPkgVersion || (t.dependencyPkgVersion = {}), void this.forEachDependencies(this.targetService.getModuleService().getAllDependencies(), (t, s) => {
      this.compiledPackageNames.includes(t) || (e.metadata.dependencyPkgVersion[t] = file_util_js_1.FileUtil.extracted(s, this.moduleModel.getParentProject().getParameterFileObj(), i));
    });
    this.targetService.isByteCodeHar() && ((s = e.metadata).dependencyPkgVersion || (s.dependencyPkgVersion = {}), this.forEachDependencies(this.targetService.getModuleService().getDirectDependencies(), (t, s) => {
      e.metadata.dependencyPkgVersion[t] = file_util_js_1.FileUtil.extracted(s, this.moduleModel.getParentProject().getParameterFileObj(), i);
    }));
  }
  traverseToSetOptimizePkgVersion(e, t, s, i) {
    e.forEach(e => {
      if (!i(e)) return;
      const r = e.getDependencyName();
      this.affectsCompilationDependencyKeys.includes(r) && (t.metadata.dependencyPkgVersion[r] = file_util_js_1.FileUtil.extracted(e.getDependencyVersion(), this.moduleModel.getParentProject().getParameterFileObj(), s));
    });
  }
  setOptimizeDependencyPkgVersion(e) {
    var t;
    if (!this.targetService.isByteCodeHar()) return;
    (t = e.metadata).dependencyPkgVersion || (t.dependencyPkgVersion = {});
    const s = {
      parameterFilePath: this.service.getProjectModel().getParameterFileAbsolutePath(),
      ohPackageFilePath: this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath()
    };
    if (this.isBundledDependencies) {
      const t = this.service.getAffectsCompilationDependencies();
      return void this.traverseToSetOptimizePkgVersion(t, e, s, e => e.isByteCodeHarDependency() || e.isOtherDependency());
    }
    const i = this.service.getAllDirectDependencies();
    this.traverseToSetOptimizePkgVersion(i, e, s, e => e.isHarDependency() || e.isOtherDependency());
  }
  validateCustomTypes(e) {
    const t = this.getDependencies(this.moduleModel, true),
      s = [];
    e.forEach(e => {
      if (t[e] || t[`@types/${e}`]) return void s.push(e);
      const i = (0, build_profile_utils_js_1.getCustomTypePathIfExists)(this.moduleModel.getProjectDir(), e);
      i && ((0, hvigor_common_1.isSubPath)(fs_extra_1.realpathSync.native(i), this.moduleModel.getProjectDir()) || s.push(e));
    }), s.length && this.logger.warn(`The files of '[${s.join(",")}]' configured at 'types' field in 'build-profile.json5' are outside of current module. Recommended to migrate these files to the current module's directory for packaging. `);
  }
  processMetadata(e) {
    var t;
    this.globalAffectsCompilationDependencyKeys.length && (e.metadata.globalDependencies = this.globalAffectsCompilationDependencyKeys), this.nonEntryImportees.length && (e.metadata.extraImportees = this.nonEntryImportees), this.isBundledDependencies && (e.metadata.compiledPackageNames = this.compiledPackageNames, e.metadata.extraImportees = this.npmImportees), this.hasSoFile() && (e.metadata.nativeDebugSymbol = this.isNativeStripped);
    !(this._harExtendInfo.isObfuscatedHar() || this.targetService.isByteCodeHar()) && this.isDependenciesTypesEnable && (null === (t = this.customTypes) || undefined === t ? undefined : t.length) && (this.validateCustomTypes(this.customTypes), e.metadata.customTypes = this.customTypes);
  }
  processPackageJson() {
    var e;
    const t = path_1.basename(this.packageManagerPath),
      s = path_1.resolve(this.taskTmpDir, t);
    if (!fs_extra_1.existsSync(s)) return;
    const i = json_util_js_1.JsonUtil.getJson5Obj(s);
    this.copyPkgTypesFilePath(i), i.metadata || (i.metadata = {});
    const r = [...((null === (e = this.targetData.getTargetOpt().source) || undefined === e ? undefined : e.sourceRoots) || []), `./${build_directory_const_js_1.BuildDirConst.SRC}/${build_directory_const_js_1.BuildDirConst.MAIN}`];
    if (Object.assign(i.metadata, {
      sourceRoots: r,
      debug: this.targetService.isDebug()
    }), this.targetService.getModuleService().getOhpmDependencyInfo(), this.isByteCodeHarOptimize ? this.setOptimizeDependencyPkgVersion(i) : this.setDependencyPkgVersion(i), this.isBundledDependencies) {
      if (Object.assign(i.metadata, {
        bundleDependencies: this.isBundledDependencies
      }), this.ohPkgMetadataResDir && 0 !== this.ohPkgMetadataResDir.length) {
        const e = [];
        this.ohPkgMetadataResDir.forEach(t => {
          e.push(path_1.normalize(t));
        });
        const t = {
          directories: e
        };
        Object.assign(i.metadata, {
          resource: t
        });
      }
      i.dependencies && this.deleteOhPkgDep(dependency_interface_js_1.DependencyEnum.DEPENDENCIES, i), i.devDependencies && delete i.devDependencies;
      i.dynamicDependencies && this.deleteOhPkgDep(dependency_interface_js_1.DependencyEnum.DYNAMIC_DEPENDENCIES, i);
    }
    this.compatibleSdkVersionStage && (i.compatibleSdkVersionStage = this.compatibleSdkVersionStage), this.processDeclarationEntry(i), this.processMetadata(i), this.processOhExports(i), fs_extra_1.writeFileSync(s, JSON.stringify(i));
  }
  processSourceMaps() {
    if (!this.targetService.isByteCodeHar()) return;
    const e = this.shouldPackSourceMaps,
      t = path_1.join(this.taskTmpDir, code_type_enum_js_1.CodeType.ETS, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP);
    if (fs_extra_1.existsSync(t)) return void (e || (fs_extra_1.removeSync(t), this.logger.debug(`Remove '${build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP}' from package dir.`)));
    if (!e) return;
    const s = this.pathInfo.getSourceMapOriginPath(this.targetService);
    fs_extra_1.existsSync(s) ? fs_extra_1.copyFileSync(s, t) : this.logger.debug(`'${build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP}' does not exist.`);
  }
  deleteOhPkgDep(e, t) {
    if (!this.targetService.isByteCodeHar() || !this.isBundledDependencies || (0, parameter_utils_js_1.getPropertyKeepDependency)()) return;
    const s = t[e];
    this.service.getAllDependencies().forEach(t => {
      const i = t.getPackageJsonObj();
      i && Object.assign(s, i[e]);
    });
    const i = this.service.getHspOrByteCodeHarDependencies().map(e => e.getDependencyName());
    Object.keys(s).forEach(e => {
      i.includes(e) || delete s[e];
    });
  }
  collectRuntimeOnlyOhmurl(e, t) {
    var s;
    const i = null === (s = har_target_util_js_1.HarTargetUtil.getDep2ConfigMap(this.targetService)) || undefined === s ? undefined : s.dep2RuntimeOnlyObj;
    if (!i) return;
    this.collectConfigOhmurl(i.dep2RuntimeOnlySources, e, t);
    const r = [];
    i.dep2RuntimeOnlyPackages.forEach(t => {
      t.forEach(t => {
        const s = this.service.getHarDependencies().find(e => e.getDependencyName() === t);
        if (!s) return;
        const i = dependency_manager_js_1.DependencyManager.getModuleNameFromOhPkgJson(s.getDependencyRootPath()),
          o = json_util_js_1.JsonUtil.getJson5Obj(s.getPackageFilePath()).main ? s.getDependencyMainFilePath() : s.getDependencyTypesFilePath(),
          a = (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(e[i], path_1.relative(`${i}`, o));
        r.push(a);
      });
    }), t.push(...r);
  }
  collectConfigOhmurl(e, t, s) {
    const i = [];
    (null == e ? undefined : e.size) && (e.forEach((e, s) => {
      const r = dependency_manager_js_1.DependencyManager.getModuleNameFromOhPkgJson(s.getDependencyRootPath());
      e.forEach(e => {
        const s = (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(t[r], e.replace(/^\.\//, ""));
        i.push(s);
      });
    }), s.push(...i));
  }
  async doTaskAction() {
    const e = this.service.getAllDependencies();
    return validate_util_js_1.ValidateUtil.validateLocalDependency(this.moduleModel, this.targetService.getBuildOption(), e), (0, validate_systemHsp_js_1.validatePackageBySystemHspModel)(this.service, this.appOptObj, this.targetJsonPath, this.targetModuleOptObj), super.doTaskAction();
  }
  getObfuscatedPageSourceFile(e, t, s) {
    const i = e.getModule().getNodeDir().substring(this.projectModel.getProjectDir().length + 1),
      r = `${i}${path_1.sep}${t.pageSourceFile}`,
      o = r.replace(path_1.extname(r), t.originalSuffix),
      a = file_util_js_1.FileUtil.normalizePathSeparator(o);
    if (!Object.prototype.hasOwnProperty.call(s, a)) return;
    const n = s[a].obfName.substring(i.length + 1);
    return n.replace(path_1.extname(n), this.hasUseTsHarField() ? ".ts" : ".js");
  }
  collectByteCodeHarAbilityOhmurl(e, t) {
    const s = res_model_loader_js_1.resModelLoader.getModuleJson(this.targetJsonPath);
    for (const i of [this.abilities, this.extensionAbilities]) {
      const r = null == s ? undefined : s.module[i];
      (null == r ? undefined : r.length) && r.forEach(s => {
        if (!(null == s ? undefined : s.srcEntry)) return;
        const r = this.targetService.getAbsoluteSrcEntryPath(s.srcEntry, this.module.getNodeDir());
        if (!fs_extra_1.existsSync(r)) return void this.logger.debug("The srcEntry path for obtaining the ability of the module is incorrect or the corresponding file does not exist.");
        if (i === this.extensionAbilities && s.srcEntry.endsWith(".so")) return;
        const o = (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(e[this.packageJsonObj.name], r.substring(this._moduleDir.length + 1));
        t.push(o);
      });
    }
  }
  collectByteCodeHarStartupTasks(e) {
    var t;
    if (!fs_extra_1.existsSync(this.intermediateTempStartupFilePath)) return;
    const s = json_util_js_1.JsonUtil.getJson5Obj(this.intermediateTempStartupFilePath);
    undefined !== s && (null === (t = s.startupTasks) || undefined === t || t.forEach(t => {
      var s;
      e.push(null !== (s = t.ohmurl) && undefined !== s ? s : t.srcEntry);
    }));
  }
  collectByteCodeHarInsightIntentSrcEntry(e, t) {
    if (!fs_extra_1.existsSync(this.insightIntentJsonPath)) return;
    if (this.processInsightIntent(e, t, this.insightIntentJsonPath, this.module.getNodeDir()), !this.isBundledDependencies) return;
    const s = new Map();
    har_target_util_js_1.HarTargetUtil.getHarPathToResourcesDirMap(s, this.targetService);
    this.service.getHarDependencies().forEach(i => {
      const r = s.get(i.getDependencyRootPath());
      r && r.forEach(s => {
        const r = i.getInsightIntentPathByResourceDir(s);
        !i.isByteCodeHarDependency() && fs_extra_1.existsSync(r) && this.processInsightIntent(e, t, this.insightIntentJsonPath, i.getDependencyRootPath());
      });
    });
  }
  processInsightIntent(e, t, s, i) {
    var r;
    const o = json_util_js_1.JsonUtil.getJson5Obj(s);
    if (!o || !o.insightIntentsSrcEntry) return;
    const a = s => {
      if (s.srcEntry) {
        const r = this.targetService.getAbsoluteSrcEntryPath(s.srcEntry, i);
        t.push((0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(e[this.packageJsonObj.name], path_1.relative(i, r)));
      }
    };
    for (const e of [...(null !== (r = o.insightIntentsSrcEntry) && undefined !== r ? r : [])]) a(e);
  }
  processObfuscatedHarAbility() {
    if (!this.moduleModel.isHarModule() || this.targetService.isOpenSource()) return;
    const e = path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, common_const_js_1.CommonConst.MODULE_JSON);
    if (!fs_extra_1.existsSync(e)) return void this.logger.debug("Failed to obtain the module.json file from the module intermediate product.");
    const t = json_util_js_1.JsonUtil.getJson5Obj(e);
    for (const e of [this.abilities, this.extensionAbilities]) {
      const s = null == t ? undefined : t.module[e];
      (null == s ? undefined : s.length) && s.forEach(e => {
        if (!(null == e ? undefined : e.srcEntry)) return;
        const t = this.getObfuscatedHarAbilitySrcEntry(e.srcEntry, this.getObfNameCacheObj());
        Object.assign(e, {
          srcEntry: t
        });
      });
    }
    fs_extra_1.writeJSONSync(e, t);
  }
  getObfuscatedHarAbilitySrcEntry(e, t) {
    const s = this.targetService.getAbsoluteSrcEntryPath(e, this.module.getNodeDir());
    if (!fs_extra_1.existsSync(s)) return void this.logger.debug("The srcEntry path for obtaining the ability of the module is incorrect or the corresponding file does not exist.");
    const i = (0, copy_resources_util_js_1.toUnixPath)(s).substring(this.projectDir.length + 1);
    if (!Object.prototype.hasOwnProperty.call(t, i)) return;
    const r = path_1.dirname(this.targetJsonPath),
      o = t[i].obfName;
    if ("" === r || !r || "" === o || !o) return;
    const a = `./${path_1.relative(r, path_1.resolve(this.projectDir, o)).replace(/\\/g, "/")}`;
    return a.replace(path_1.extname(a), this.hasUseTsHarField() ? ".ts" : ".js");
  }
  getObfNameCacheObj() {
    const e = path_1.resolve(this.pathInfo.getModuleBuildCachePath(), `${this.targetName}@HarCompileArkTS`, "esmodule", "release", "obfuscation", "nameCache.json");
    return json_util_js_1.JsonUtil.getJson5Obj(e);
  }
  mergeDeclarationEntry(e, t) {
    var s, i;
    null === (i = null === (s = e.metadata) || undefined === s ? undefined : s.declarationEntry) || undefined === i || i.forEach(e => {
      t.includes(e) || t.push(e);
    });
  }
  processOhExports(e) {
    e[common_const_js_1.CommonConst.OH_EXPORTS] && (this.targetService.isByteCodeHar() ? e[common_const_js_1.CommonConst.OH_EXPORTS] = (0, oh_exports_util_js_1.rewriteOhExports)(this.moduleModel.getProjectDir(), this.declaredFilesOutputDir, e[common_const_js_1.CommonConst.OH_EXPORTS]) : this._harExtendInfo.isObfuscatedHar() && (e[common_const_js_1.CommonConst.OH_EXPORTS] = (0, oh_exports_util_js_1.rewriteOhExports)(this.moduleModel.getProjectDir(), this.arkTsCompiledOutputDir, e[common_const_js_1.CommonConst.OH_EXPORTS])));
  }
}
__decorate([(0, hvigor_1.Input)()], ProcessHarArtifacts.prototype, "affectsCompilationDependencyKeys", undefined), __decorate([(0, hvigor_1.Input)()], ProcessHarArtifacts.prototype, "globalAffectsCompilationDependencyKeys", undefined), __decorate([(0, hvigor_1.Input)()], ProcessHarArtifacts.prototype, "isByteCodeHarOptimize", null), __decorate([(0, hvigor_1.Input)()], ProcessHarArtifacts.prototype, "shouldPackSourceMaps", null), __decorate([(0, hvigor_1.InputFile)()], ProcessHarArtifacts.prototype, "insightIntentJsonPath", null), __decorate([(0, hvigor_1.Input)()], ProcessHarArtifacts.prototype, "customTypes", null), __decorate([(0, hvigor_1.InputFile)()], ProcessHarArtifacts.prototype, "preloadSoFilePath", null), __decorate([(0, hvigor_1.Input)()], ProcessHarArtifacts.prototype, "isDependenciesTypesEnable", null);
