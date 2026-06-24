/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/builder/app-plugin-builder.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as collect_duplicate_har_dependencies_js_1 from "../../har-dedepulicate/collect_duplicate_har_dependencies";
import * as config_file_loader_js_1 from "../../utils/loader/config-file-loader";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as one_sdk_validator_js_1 from "../../utils/one-sdk-validator";
import * as resolver_cache_js_1 from "../../utils/resolver/resolver-cache";
import * as custom_hook_service_js_1 from "../hooks/custom-hook-service";
import * as module_incremental_skip_optimization_js_1 from "../hooks/module-incremental-skip-optimization";
import * as ohpm_load_install_js_1 from "../hooks/ohpm-load-install";
import * as task_schedule_optimization_js_1 from "../hooks/task-schedule-optimization";
import * as project_task_initializer_js_1 from "../task-initializer/project-task-initializer";
import * as abstract_plugin_builder_js_1 from "./abstract-plugin-builder";
export class AppPluginBuilder extends abstract_plugin_builder_js_1.AbstractPluginBuilder {
  constructor(e, i) {
    super(i), this.project = e;
  }
  prepare() {
    this._log.debug("App plugin prepare start..."), config_file_loader_js_1.configFileLoader.clean(), res_model_loader_js_1.resModelLoader.clean(), oh_package_loader_js_1.ohPackageLoader.clean(), custom_hook_service_js_1.customHookActionService.clean(), this._log.debug("App plugin prepare end...");
  }
  createPlugin() {
    return this._log.debug("App plugin creating..."), this.initializationStrategy.createAppPlugin(this.project);
  }
  registerHooks(e) {
    this._log.debug("App plugin register hook start..."), this.registerAfterBindSystemPluginsHook(e), this.registerNodeEvaluatedInternalHook(e), hvigor_1.hvigorCore.hvigorNodeEvaluatedInternalHook(() => {
      (0, collect_duplicate_har_dependencies_js_1.processHarDeduplicate)(e);
    }), this.registerOhpmLoadInstallHook(e), this.registerModuleSkipOptimizationHook(this.project), this.registerTaskScheduleOptimizationHook(this.project), this._log.debug("App plugin register hook end...");
  }
  registerAfterBindSystemPluginsHook(e) {
    this.project.afterBindSystemPlugins(async () => {
      var i;
      e.initGlobalData(), e.initBuildOptionMap(e._project.getBuildProfilePath()), e.initTaskService(), await (null === (i = e.getTaskService()) || undefined === i ? undefined : i.setup()), e.initContext(), project_task_initializer_js_1.ProjectTaskInitializer.initialize(e);
    });
  }
  registerNodeEvaluatedInternalHook(e) {
    hvigor_1.hvigorCore.hvigorNodeEvaluatedInternalHook(() => {
      var i, o;
      this.ifThrowBuildModeError(), resolver_cache_js_1.ResolverCache.clear(), null === (i = e.getTaskService()) || undefined === i || i.initDependencyInfo(), this.initializeTaskDepends(this.project, undefined, false), one_sdk_validator_js_1.OneSdkValidator.apiInspection(e.getProjectModel()), e.initTraceData(), null === (o = e.getTaskService()) || undefined === o || o.initProductData();
    });
  }
  registerOhpmLoadInstallHook(e) {
    hvigor_1.hvigorCore.hvigorNodeBeforeEvaluatedInternalHook(() => {
      new ohpm_load_install_js_1.OhpmLoadInstallService().ohpmLoadInstall(e);
    });
  }
  registerModuleSkipOptimizationHook(e) {
    "true" === process.env.ENABLE_MODULE_SKIP && hvigor_1.hvigor.taskGraphResolvedInternal(() => {
      new module_incremental_skip_optimization_js_1.ModuleIncrementalSkipService().processModuleSkip(e);
    });
  }
  registerTaskScheduleOptimizationHook(e) {
    true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(hvigor_1.INCREMENTAL_OPTIMIZATION) && hvigor_1.hvigor.taskGraphResolvedInternal(() => {
      new task_schedule_optimization_js_1.TaskScheduleOptimizationService(e).computeAndSetCompileDependencies();
    });
  }
}
