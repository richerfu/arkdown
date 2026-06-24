/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sync/ide/ohos-module-model-bean.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as find_target_product_js_1 from "../../../common/find-target-product";
import * as build_directory_const_js_1 from "../../../const/build-directory-const";
import * as common_const_js_1 from "../../../const/common-const";
import * as code_type_enum_js_1 from "../../../enum/code-type-enum";
import * as compile_mode_enum_js_1 from "../../../enum/compile-mode-enum";
import * as module_type_enum_js_1 from "../../../enum/module-type-enum";
import * as ohos_plugin_id_js_1 from "../../../plugin/common/ohos-plugin-id";
import * as build_mode_manager_js_1 from "../../../project/build-option/build-mode-manager";
import * as compile_resources_util_js_1 from "../../../utils/compile-resources-util";
import * as inject_util_js_1 from "../../../utils/inject-util";
import * as har_module_build_profile_loader_js_1 from "../../../utils/loader/file/har-module-build-profile-loader";
import * as module_build_profile_loader_js_1 from "../../../utils/loader/file/module-build-profile-loader";
import * as ohos_logger_js_1 from "../../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../../utils/task-util";
import * as task_names_js_1 from "../../common/task-names";
import * as compile_node_js_1 from "../../compile-node";
import * as module_task_service_js_1 from "../../service/module-task-service";
import * as target_task_service_js_1 from "../../service/target-task-service";
const _log = ohos_logger_js_1.OhosLogger.getLogger("OhosModuleModelBean"),
  ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET,
  moduleType2IdMap = new Map([[module_type_enum_js_1.ModuleType.Har, ohos_plugin_id_js_1.OhosPluginId.OHOS_HAR_PLUGIN], [module_type_enum_js_1.ModuleType.Entry, ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN], [module_type_enum_js_1.ModuleType.Feature, ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN], [module_type_enum_js_1.ModuleType.Shared, ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN]]);
export class OhosModuleModelBean {
  constructor(e, t) {
    this.modelId = "ohos-module", this._isFaMode = t, this._module = e;
    const o = e.getProject().getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_APP_PLUGIN);
    this._projectModel = o.getProjectModel(), this._moduleModel = this._projectModel.getModuleModelByName(this._module.getName()), this._moduleType = this._moduleModel.getModuleType(), this.modelId = this.modelId.concat("-").concat(e.getName()), this._isHarModule = this._moduleType === module_type_enum_js_1.ModuleType.Har;
    const _ = moduleType2IdMap.get(this._moduleModel.getModuleType()),
      s = e.getPluginById(_);
    s || _log.printErrorExit("THE_%S_PLUGIN_WAS_NOT_FOUND_WHEN_INITIALIZING_THE_%S_MODULE", [_, e.getName()]), this._moduleTaskService = s.getTaskService(), this._targetTaskServices = s.getNeedExecTargetServiceList(), this._targetDataSet = this._moduleTaskService.getTargetDataSet();
  }
  computeCommonInfo(e) {
    e.set("SELECT_TARGET", common_const_js_1.DefaultTargetConst.DEFAULT_TARGET);
    const t = path_1.resolve(this._module.getNodeDir(), common_const_js_1.CommonConst.BUILD),
      o = inject_util_js_1.InjectUtil.getBuildCacheParentDir(t, path_1.join(hvigor_1.hvigorCore.getProject().getName(), t.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length)));
    e.set("MODULE_BUILD_DIR", path_1.resolve(o));
  }
  computeBuildOption(e) {
    const t = {};
    this._targetTaskServices.forEach(e => {
      const o = (0, find_target_product_js_1.findTargetProduct)(this._projectModel).name.concat("-").concat(e.getTargetData().getTargetName());
      t[o] = e.getBuildOption();
    }), e.set("BUILD_OPTION", t), this._isHarModule ? e.set("BUILD_PROFILE_OPT", har_module_build_profile_loader_js_1.harModuleBuildProfileLoader.getBuildProfile(this._moduleModel.getName())) : e.set("BUILD_PROFILE_OPT", module_build_profile_loader_js_1.moduleBuildProfileLoader.getBuildProfile(this._moduleModel.getName()));
  }
  computePathInfo(e) {
    const t = new Map();
    this._targetDataSet.forEach(e => {
      var o;
      const _ = e[0],
        s = _.getTargetName(),
        a = new Map(),
        r = _.getPathInfo(),
        i = this._moduleTaskService.getModuleModel(),
        l = this.getCachePath(r, _, i),
        d = path_1.resolve(r.getIntermediatesLoaderPath(), build_directory_const_js_1.BuildArtifactConst.LOADER_JSON),
        u = this._moduleTaskService.getHarDependencies(),
        n = new Map(),
        m = new Map();
      u.forEach((e, t) => {
        const o = path_1.resolve(e.getDependencyRootPath(), "src", "main", build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES),
          _ = compile_resources_util_js_1.CompileResourcesUtil.getCompileTargetFolderBySeq(build_directory_const_js_1.BuildDirConst.HAR_COMPILED, t);
        n.set(o, _);
      });
      const g = {
        OUTPUT_PATH: r.getModuleBuildOutputPath(),
        INTERMEDIA_PATH: r.getModuleBuildIntermediates(),
        JS_ASSETS_PATH: r.getInterMediatesLoaderOutPath(),
        JS_LITE_ASSETS_PATH: r.getInterMediatesLoaderOutLitePath(),
        RES_PATH: path_1.resolve(r.getIntermediatesRes(), this.getFirstEntryName()),
        RES_PROFILE_PATH: r.getIntermediatesResProfilePath(),
        ETS_SUPER_VISUAL_PATH: l.etsLoaderCachePath,
        JS_SUPER_VISUAL_PATH: l.aceLoaderCachePath,
        WORKER_LOADER: d,
        MANIFEST_JSON: r.getIntermediatesLegacyManifestJson(),
        OUTPUT_METADATA_JSON: this._moduleType === module_type_enum_js_1.ModuleType.Shared ? r.getIntermediatesHspOutputMetadata() : r.getIntermediatesOutputMetadata(),
        SOURCE_MAP_DIR: r.getInterMediatesSourceMapDirPath()
      };
      a.set("SOURCE_ROOT", null == i ? undefined : i.getSourceRootByTargetName(s));
      const c = [];
      c.push(null == i ? undefined : i.getSourceSetByTargetName(s).getTargetResPath()), c.forEach((e, t) => {
        const o = path_1.isAbsolute(e) ? e : path_1.resolve(i.getProjectDir(), e),
          _ = compile_resources_util_js_1.CompileResourcesUtil.getCompileTargetFolderBySeq(build_directory_const_js_1.BuildDirConst.MODULE_COMPILED, t);
        m.set(o, _);
      });
      const h = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(this._moduleModel.getName(), s);
      a.set("RESOURCES_PATH", c), a.set("BUILD_PATH", g), a.set("BUILD_OPTION", {
        debuggable: null === (o = h.debuggable) || undefined === o || o
      }), t.set(s, a);
    }), e.set("TARGETS", t);
  }
  getCachePath(e, t, o) {
    const _ = t.getTargetName();
    let s = this._targetTaskServices.find(e => e.getTargetData().getTargetName() === _);
    _ === ohosTestTargetName && (s = this._targetTaskServices.find(e => e.getTargetData().getTargetName() !== _));
    const a = !s || s.getSdkInfo().hasRollUpPluginInEtsLoader;
    let r = a ? task_names_js_1.TaskNames.LegacyFATask.COMPILE_ARK.name : task_names_js_1.TaskNames.LegacyFATask.COMPILE_NODE.name,
      i = a ? this._isHarModule ? task_names_js_1.TaskNames.Task.HAR_COMPILE.name : task_names_js_1.TaskNames.Task.COMPILE_ARK.name : task_names_js_1.TaskNames.Task.COMPILE_NODE.name;
    _ === ohosTestTargetName && (r = a ? task_names_js_1.TaskNames.LegacyFATask.OHOS_TEST_ARK_COMPILE.name : task_names_js_1.TaskNames.LegacyFATask.OHOS_TEST_COMPILE_NODE.name, i = a ? task_names_js_1.TaskNames.Task.OHOS_TEST_ARK_COMPILE.name : task_names_js_1.TaskNames.Task.OHOS_TEST_COMPILE_NODE.name);
    const l = this._isFaMode ? r : i,
      d = (0, target_task_service_js_1.computeTargetTaskName)(t.getTargetName(), compile_node_js_1.CompileNode.getCompileNodeTaskName(code_type_enum_js_1.CodeType.ETS, l)),
      u = path_1.resolve(e.getModuleBuildCachePath(), d, `${o.getCompileMode(t.getProduct())}`),
      n = (0, target_task_service_js_1.computeTargetTaskName)(t.getTargetName(), compile_node_js_1.CompileNode.getCompileNodeTaskName(code_type_enum_js_1.CodeType.JS, l));
    return {
      etsLoaderCachePath: u,
      aceLoaderCachePath: path_1.resolve(e.getModuleBuildCachePath(), n, `${compile_mode_enum_js_1.CompileModeEnum.JS_BUNDLE}`)
    };
  }
  getFirstEntryName() {
    const e = this._moduleTaskService.getRelatedEntryModules(),
      t = this._moduleTaskService.getModuleModel().getModuleType();
    return module_task_service_js_1.ModuleTaskService.checkEntryModules(t, e) && this._moduleTaskService.isFaMode() ? e[0] : "";
  }
  computeBuildDirInfo(e) {
    var t;
    let o = null !== (t = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_CACHE_DIR)) && undefined !== t ? t : hvigor_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.CommonConst.BUILD_DIR_KEY);
    undefined !== o && fs_extra_1.existsSync(o) && (o = fs_extra_1.realpathSync.native(o)), e.set("BUILD_CACHE_DIR", o || "");
  }
  buildModel() {
    const e = new Map();
    return this.computeCommonInfo(e), this.computePathInfo(e), this.computeBuildOption(e), this.computeBuildDirInfo(e), (0, hvigor_common_1.strMapToObj)(e);
  }
  taskInfo() {
    const e = [];
    return this._module.getAllTasksCA().forEach(t => {
      e.push((0, task_util_js_1.getTaskInfoFromTaskCA)(t));
    }), this._module.getAllTasks().forEach(t => {
      e.push((0, task_util_js_1.getTaskInfoFromTask)(t));
    }), e;
  }
}
