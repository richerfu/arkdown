/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/task-util.js
 */
import * as path from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as find_target_product_js_1 from "../common/find-target-product";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as build_mode_const_js_1 from "../const/build-mode-const";
import * as common_const_js_1 from "../const/common-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as task_names_js_1 from "../tasks/common/task-names";
import * as legacy_compile_resource_js_1 from "../tasks/legacy-tasks/legacy-compile-resource";
import * as legacy_preview_compile_resource_js_1 from "../tasks/legacy-tasks/legacy-preview-compile-resource";
import * as legacy_process_resource_increment_js_1 from "../tasks/legacy-tasks/legacy-process-resource-increment";
import * as legacy_process_resource_js_1 from "../tasks/legacy-tasks/legacy-process-resource";
import * as module_task_service_js_1 from "../tasks/service/module-task-service";
import * as har_target_util_js_1 from "./har-target-util";
import * as json_util_js_1 from "./json-util";
export const resolveHookDependency = (e, s) => `${s}@${e}`;
export const dynamicResolveResourceTasks = (e, s) => {
  const o = e.getModuleService(),
    t = o.getModuleModel().getModuleType(),
    r = o.getRelatedEntryModules();
  if (module_task_service_js_1.ModuleTaskService.checkEntryModules(t, r)) for (const o of r) legacyResourceTask(e, s, o);
  legacyResourceTask(e, s);
};
const legacyResourceTask = (e, s, o) => {
    const t = new legacy_process_resource_increment_js_1.LegacyProcessResourceIncrement(e, o),
      r = new legacy_preview_compile_resource_js_1.LegacyPreviewCompileResource(e, o);
    r.declareDepends(t), s.registry(t), s.registry(r);
    const i = new legacy_process_resource_js_1.LegacyProcessResource(e, o),
      n = new legacy_compile_resource_js_1.LegacyCompileResource(e, o);
    n.declareDepends(i), s.registry(i), s.registry(n);
  },
  warnCreateBuildProfileTask = (e, s, o) => {
    const t = e.getCompileApiVersion(),
      r = s.getBuildProfileFields();
    t === sdk_const_js_1.ApiVersion.API_VERSION_9 && undefined !== r && o.warn('Field "buildProfileFields" is not supported in API version 9. The value of this field will be ignored.');
  };
export const limitMinApiVersion = (e, s) => {
  const o = e.getCompileApiVersion(),
    t = e.getCompatibleApiVersion();
  return o > s && t > s;
};
export const declareReloadDepends = e => {
  const s = (0, limitMinApiVersion)(e.getTargetData(), sdk_const_js_1.ApiVersion.API_VERSION_9),
    o = ["buildHotReloadResource"];
  if (s) {
    o.push(`${e.getTargetData().getTargetName()}@${task_names_js_1.TaskNames.CommonTask.CREATE_BUILD_PROFILE.name}`);
    har_target_util_js_1.HarTargetUtil.calDepHarTargets(e).forEach((e, s) => o.push(`${s}:${e}@${task_names_js_1.TaskNames.CommonTask.CREATE_HAR_BUILD_PROFILE.name}`));
  }
  return o;
};
export const getBuildModeName = () => {
  const e = hvigor_1.hvigorCore.getExtraConfig().get("buildMode");
  return e || (hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name) ? build_mode_const_js_1.BuildModeConst.RELEASE : build_mode_const_js_1.BuildModeConst.DEBUG);
};
export const shouldCompileArkTsWidget = (e, s) => !s && e.some(e => {
  var s;
  const o = json_util_js_1.JsonUtil.getJson5Obj(e);
  return !!o && (null === (s = o.forms) || undefined === s ? undefined : s.some(e => "arkts" === e.uiSyntax));
});
export const doObfuscationCheck = (e, s) => {
  var o, t, r, i;
  if (e.isDebug()) return;
  false !== (null === (i = null === (r = null === (t = null === (o = e.getBuildOption()) || undefined === o ? undefined : o.arkOptions) || undefined === t ? undefined : t.obfuscation) || undefined === r ? undefined : r.ruleOptions) || undefined === i ? undefined : i.enable) || s.warn("If obfuscation is needed, enable obfuscation settings in this build process; failing to do so may prevent future obfuscation. \n               Properly configure obfuscation rules to avoid runtime issues.");
};
export const getBundleType = (e, s) => {
  var o, t, r;
  const i = null === (o = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === o ? undefined : o.appOpt;
  return null !== (r = null !== (t = null == i ? undefined : i.bundleType) && undefined !== t ? t : e.bundleType) && undefined !== r ? r : s.getBundleType();
};
export const isAtomicServiceProject = (e, s) => (0, getBundleType)(e, s) === common_const_js_1.BundleType.ATOMIC_SERVICE;
export const checkAppPreloadHintStartupTasksSrcEntry = (e, s, o, t, r = false) => {
  if (!fs_extra_1.existsSync(e)) return;
  const i = fs_extra_1.readJsonSync(s).libs,
    n = Object.keys(i).map(e => path.basename(e)),
    a = json_util_js_1.JsonUtil.getJson5Obj(e);
  undefined !== (null == a ? undefined : a.appPreloadHintStartupTasks) && a.appPreloadHintStartupTasks.forEach(e => {
    n.includes(path.basename(e.srcEntry)) || r && e.isRemoteByteCodeHar || o.printErrorExit("STARTUP_SO_FILE_EXIST", [e.srcEntry, t, e.startupConfigPath], [[e.srcEntry]]);
  });
};
export const isFastBuildApp = e => {
  var s;
  const o = null === (s = (0, find_target_product_js_1.findTargetProduct)(e.getProjectModel()).buildOption) || undefined === s ? undefined : s.packOptions;
  return (null == o ? undefined : o.fastBuildApp) && hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name) || false;
};
export const isHomeAbility = e => {
  var s;
  return !(!e.srcEntry || !(null === (s = e.skills) || undefined === s ? undefined : s.length)) && e.skills.some(e => (e.entities || []).some(e => e === common_const_js_1.CommonConst.DEFAULT_ENTITY) && (e.actions || []).some(e => e === common_const_js_1.CommonConst.DEFAULT_ACTION || e === common_const_js_1.CommonConst.DEFAULT_WANT_ACTION));
};
export const getTaskInfoFromTask = e => {
  var s;
  return {
    TASK_NAME: e.getName(),
    GROUP: e.getGroup(),
    DESCRIPTION: null !== (s = e.getDescription()) && undefined !== s ? s : "",
    SOURCE_PLUGIN_ID: e.getSourcePluginId()
  };
};
export const getTaskInfoFromTaskCA = e => {
  var s, o, t;
  return {
    TASK_NAME: e.details.name,
    GROUP: null !== (s = e.details.group) && undefined !== s ? s : "",
    DESCRIPTION: null !== (o = e.details.description) && undefined !== o ? o : "",
    SOURCE_PLUGIN_ID: null !== (t = e.details.sourcePluginId) && undefined !== t ? t : ""
  };
};
export { warnCreateBuildProfileTask };
