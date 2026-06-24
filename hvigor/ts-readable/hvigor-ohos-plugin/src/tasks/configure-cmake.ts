/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/configure-cmake.js
 */
import fs_extra_1 from "fs-extra";
import os_1 from "os";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as cmake_util_js_1 from "../utils/cmake/cmake-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as native_cmake_resolver_js_1 from "../utils/native-cmake-resolver";
import * as task_names_js_1 from "./common/task-names";
import * as legacy_pre_build_js_1 from "./legacy-tasks/legacy-pre-build";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
import * as pre_build_js_1 from "./pre-build";
const _log = ohos_logger_js_1.OhosLogger.getLogger("ConfigureCmake");
export class ConfigureCmake extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    var t;
    super(e, task_names_js_1.TaskNames.Task.CONFIGURE_CMAKE), this.dependencies = [], this.packages = [], this.nativeOption = e.getBuildOption().externalNativeOptions, this.abiList = cmake_util_js_1.CmakeUtil.checkAbiFilters(null === (t = this.nativeOption) || undefined === t ? undefined : t.abiFilters, e.getTargetData().isHarmonyOS(), this.moduleModel, this.targetName), this.cmakeDir = path_1.resolve(this.pathInfo.getNinjaWorkDir(), "hvigor"), this.init();
  }
  taskShouldDo() {
    return cmake_util_js_1.CmakeUtil.nativeTaskCondition(this.moduleModel, this.targetData, this.nativeOption);
  }
  init() {
    for (const e of this.service.getDependencyInfo().getNpmDependencies()) {
      const t = this.isLocal(e);
      this.isNativeLibrary(e, t) && (_log.debug(`Resolve native package ${e.getDependencyName()}.`), this.dependencies.push(e), this.packages.push(native_cmake_resolver_js_1.PackageResolver.getInstance(e, this.abiList, this.cmakeDir, t)));
    }
  }
  async doTaskAction() {
    const e = [];
    for (const t of this.packages) {
      const a = await t.build();
      e.push(...a);
    }
    await this.writeCmakeScript(e), await this.genCmakeFindFile();
  }
  async writeCmakeScript(e) {
    var t;
    const a = new Map();
    for (const s of e) {
      const e = null !== (t = a.get(s.cmakeFile)) && undefined !== t ? t : [];
      e.push(...s.libraries), a.set(s.cmakeFile, e);
    }
    for (const [e, t] of a.entries()) {
      const a = t.map(e => e.text).join(os_1.EOL);
      if (!fs_extra_1.existsSync(e)) {
        await fs_extra_1.ensureDir(path_1.dirname(e)), await fs_extra_1.writeFile(e, a);
        continue;
      }
      (await fs_extra_1.readFile(e, {
        encoding: "utf8"
      })) !== a && (await fs_extra_1.rm(e, {
        force: true
      }), await fs_extra_1.writeFile(e, a));
    }
  }
  async genCmakeFindFile() {
    for (const e of this.abiList) {
      const t = this.packages.map(t => t.archs.find(t => t.arch === e)).filter(e => undefined !== e).map(e => e ? `if(EXISTS ${e.cmakeFile.replace(/\\/g, "/")})${os_1.EOL}    find_package(${e.pkg.name} REQUIRED CONFIG)${os_1.EOL}endif()${os_1.EOL}` : "").join(os_1.EOL),
        a = path_1.resolve(this.cmakeDir, e, common_const_js_1.NativeConst.PACKAGE_SUMMARY_FILE);
      if (fs_extra_1.existsSync(a)) {
        if ((await fs_extra_1.readFile(a, {
          encoding: "utf8"
        })) === t) return;
      }
      await fs_extra_1.ensureDir(path_1.dirname(a)), await fs_extra_1.writeFile(a, t);
    }
  }
  initTaskDepends() {
    var e;
    this.declareDepends(this.isFaMode ? legacy_pre_build_js_1.LegacyPreBuild.name : pre_build_js_1.PreBuild.name);
    const t = this.targetService.getModuleService();
    for (const a of t.getDependencyInfo().getModuleDependencyMap().keys()) {
      const t = this.getModuleTarget(a),
        s = null !== (e = null == t ? undefined : t.getTargetData().getTargetName()) && undefined !== e ? e : "default";
      this.declareDepends(`${s}@${task_names_js_1.TaskNames.Task.BUILD_NATIVE_WITH_CMAKE.name}`, a);
    }
  }
  isLocal(e) {
    if (!e.isLocal()) return;
    const t = this.service.getModuleDependencies().find(t => path_1.resolve(t.getDependencyRootPath()) === e.getDependencyRootPath());
    return t ? this.getModuleTarget(t.getModuleName()) : undefined;
  }
  getModuleTarget(e) {
    var t, a;
    const s = this.targetService.getModuleService().getProjectModel();
    return null !== (a = null !== (t = s.getTarget(e, this.targetName)) && undefined !== t ? t : s.getTarget(e, common_const_js_1.DefaultTargetConst.DEFAULT_TARGET)) && undefined !== a ? a : s.getTarget(e);
  }
  isNativeLibrary(e, t) {
    return t ? cmake_util_js_1.CmakeUtil.nativeTaskCondition(t.getModuleService().getModuleModel(), t.getTargetData(), t.getBuildOption().externalNativeOptions) : fs_extra_1.existsSync(path_1.resolve(e.getDependencyRootPath(), "libs")) && fs_extra_1.readdirSync(path_1.resolve(e.getDependencyRootPath(), "libs")).length > 0 || fs_extra_1.existsSync(path_1.resolve(e.getDependencyRootPath(), "include"));
  }
}
