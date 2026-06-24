/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/native-task-initializer.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../../const/common-const";
import * as cmake_util_js_1 from "../../utils/cmake/cmake-util";
import * as har_target_util_js_1 from "../../utils/har-target-util";
import * as hsp_target_utils_js_1 from "../../utils/hsp-target-utils";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as build_native_with_cmake_js_1 from "../build-native-with-cmake";
import * as build_native_with_ninja_js_1 from "../build-native-with-ninja";
import * as cache_native_libs_js_1 from "../cache-native-libs";
import * as task_names_js_1 from "../common/task-names";
import * as configure_cmake_js_1 from "../configure-cmake";
import * as do_native_binxo_js_1 from "../do-native-binxo";
import * as do_native_strip_js_1 from "../do-native-strip";
import * as prepare_quickfix_js_1 from "../prepare-quickfix";
import * as process_libs_js_1 from "../process-libs";
import * as task_creator_js_1 from "../task-creator";
export const initNativeTasks = (e, s, t = false) => {
  e.registry(new NinjaCA(s, t)), e.registry(new CmakeCA(s, t)), e.registry(new ProcessLibsCA(s, t)), e.registry(new DoNativeBinxoCA(s, t)), e.registry(new PrepareQuickfixCA(s)), e.registry(new DoNativeStripCA(s, t)), e.registry(new CacheNativeLibsCA(s, t)), e.registry(new CmakeConfigureCA(s, t));
};
export class CmakeCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [configure_cmake_js_1.ConfigureCmake.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.BUILD_NATIVE_WITH_CMAKE, this.provider = () => new build_native_with_cmake_js_1.BuildNativeWithCmake(this.targetService);
  }
}
export class NinjaCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new build_native_with_ninja_js_1.BuildNativeWithNinja(this.targetService), this.declareDepends = () => {
      var e;
      cmake_util_js_1.CmakeUtil.checkAbiFilters(null === (e = this.targetService.getBuildOption().externalNativeOptions) || undefined === e ? undefined : e.abiFilters, this.targetService.getTargetData().isHarmonyOS(), this.targetService.getModuleService().getModuleModel(), this.targetService.getTargetData().getTargetName());
      const s = [build_native_with_cmake_js_1.BuildNativeWithCmake.name];
      return getModuleDependencies(this.targetService, task_names_js_1.TaskNames.Task.DO_NATIVE_STRIP.name, s), s;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.BUILD_NATIVE_WITH_NINJA;
  }
}
export class ProcessLibsCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [build_native_with_ninja_js_1.BuildNativeWithNinja.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_LIB, this.provider = () => new process_libs_js_1.ProcessLibs(this.targetService);
  }
}
export class DoNativeBinxoCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [process_libs_js_1.ProcessLibs.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.DO_NATIVE_BINXO, this.provider = () => new do_native_binxo_js_1.DoNativeBinxo(this.targetService);
  }
}
export class PrepareQuickfixCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => ["ColdReloadArkTS", do_native_strip_js_1.DoNativeStrip.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PREPARE_QUICKFIX, this.provider = () => new prepare_quickfix_js_1.PrepareQuickfix(this.targetService);
  }
}
export class DoNativeStripCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => isBinxoEnable(this.targetService) ? [do_native_binxo_js_1.DoNativeBinxo.name] : [process_libs_js_1.ProcessLibs.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.DO_NATIVE_STRIP, this.provider = () => new do_native_strip_js_1.DoNativeStrip(this.targetService);
  }
}
export class CacheNativeLibsCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [task_names_js_1.TaskNames.Task.DO_NATIVE_STRIP.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.CACHE_NATIVE_LIBS, this.provider = () => new cache_native_libs_js_1.CacheNativeLibs(this.targetService);
  }
}
export class CmakeConfigureCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => {
      const e = [task_names_js_1.TaskNames.Task.PRE_BUILD.name];
      return getModuleDependencies(this.targetService, task_names_js_1.TaskNames.Task.BUILD_NATIVE_WITH_CMAKE.name, e), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.CONFIGURE_CMAKE, this.provider = () => new configure_cmake_js_1.ConfigureCmake(this.targetService);
  }
}
function getModuleDependencies(e, s, t) {
  hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(e).forEach((e, a) => t.push(`${a}:${e.getTargetName()}@${s}`));
  return har_target_util_js_1.HarTargetUtil.calDepHarTargets(e).forEach((e, a) => t.push(`${a}:${e}@${s}`)), t;
}
function getPatchConfigMode(e) {
  var s;
  const t = path_1.resolve(inject_util_js_1.InjectUtil.getBuildCacheParentDir(e, path_1.join(hvigor_1.hvigorCore.getProject().getName(), e.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length)), true), "build"),
    a = path_1.resolve(t, "config", "buildConfig.json");
  if (fs_extra_1.existsSync(a)) {
    const e = fs_extra_1.readJsonSync(a);
    return null === (s = null == e ? undefined : e.patchConfig) || undefined === s ? undefined : s.mode;
  }
}
function isBinxoEnable(e) {
  var s;
  const t = cmake_util_js_1.CmakeUtil.getCMakeArguments(null === (s = e.getBuildOption().externalNativeOptions) || undefined === s ? undefined : s.arguments),
    a = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonCppConst.OHOS_ENABLE_BINXO),
    i = t.includes("-DOHOS_ENABLE_BINXO=ON");
  return a ? "true" === a : i;
}
