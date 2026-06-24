/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/builder/abstract-plugin-builder.js
 */
import * as stage_init_strategy_js_1 from "../strategy/stage-init-strategy";
import * as fa_init_strategy_js_1 from "../strategy/fa-init-strategy";
import * as ohos_module_task_js_1 from "../../tasks/task/ohos-module-task";
import * as global_project_data_service_js_1 from "../../tasks/service/global-project-data-service";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class AbstractPluginBuilder {
  constructor(t) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(AbstractPluginBuilder.name), this.isFaMode = t, this.initializationStrategy = this.createInitializationStrategy();
  }
  build() {
    this._log.debug("Starting plugin build process"), this.prepare();
    const t = this.createPlugin();
    return this.registerHooks(t), this._log.debug("Plugin build process completed"), t;
  }
  prepare() {}
  initializeTaskDepends(t, e, i) {
    i || !e ? t.getTaskContainer().getAllTasks().filter(t => t instanceof ohos_module_task_js_1.OhosModuleTask).forEach(t => t.initTaskDepends()) : null == e || e.configure();
  }
  ifThrowBuildModeError() {
    const t = global_project_data_service_js_1.GlobalProjectDataService.getInstance().getBuildModeInfo(),
      e = t.buildModeName,
      i = t.buildProfilePath;
    t.ifThrowError && this._log.printErrorExit("BUILD_MODE_NOT_DEFINED", [e, i], [[e, i]]);
  }
  createInitializationStrategy() {
    return this.isFaMode ? new fa_init_strategy_js_1.FaInitializationStrategy() : new stage_init_strategy_js_1.StageInitializationStrategy();
  }
}
