/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/hvigor-node-impl/default-node-impl.js
 */
import path_1 from "path";
import * as hook_const_js_1 from "../../../../common/const/hook-const";
import * as noop_js_1 from "../../../../common/util/noop";
import * as queue_js_1 from "../../../../common/util/queue";
import * as class_type_const_js_1 from "../../../common/options/class-type-const";
import * as common_const_js_1 from "../../../common/options/common-const";
import * as hvigor_core_js_1 from "../../../external/core/hvigor-core";
import * as default_plugin_container_js_1 from "../../../external/plugin/default-plugin-container";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as loader_profile_js_1 from "../../../vigor/plugin/interface/loader-profile";
import * as plugin_context_js_1 from "../../../vigor/plugin/interface/plugin-context";
import * as transform_hvigor_task_js_1 from "../../../vigor/task/impl/transform-hvigor-task";
import * as default_internal_task_js_1 from "../../task/core/default-internal-task";
import * as lazy_task_container_js_1 from "../../task/core/lazy-task-container";
import * as task_directed_acyclic_graph_js_1 from "../../task/core/task-directed-acyclic-graph";
import * as task_path_util_js_1 from "../../task/util/task-path-util";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
export class DefaultNodeImpl {
  constructor(t, e) {
    this.classKind = class_type_const_js_1.ClassTypeConst.HVIGOR_NODE, this.taskMap = new Map(), this.extraOption = new Map(), this.logger = hvigor_log_js_1.HvigorLogger.getLogger(DefaultNodeImpl.name), this.beforeNodeEvaluateFnQueue = new queue_js_1.Queue(), this.afterNodeEvaluateFnQueue = new queue_js_1.Queue(), this.afterBindSystemPluginsFnQueue = new queue_js_1.Queue(), this._nodeName = t, this._nodePath = e, this._packageJsonPath = path_1.resolve(e, common_const_js_1.HvigorCommonConst.PACKAGE_JSON), this._buildFilePath = path_1.resolve(e, common_const_js_1.HvigorCommonConst.BUILD_FILE_NAME), this._buildProfilePath = path_1.resolve(e, common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE), this._taskGraph = new task_directed_acyclic_graph_js_1.TaskDirectedAcyclicGraph(), this._tasks = new lazy_task_container_js_1.LazyTaskContainer(t), this._pluginContainer = new default_plugin_container_js_1.DefaultPluginContainer(), this._currentNodeLoaderPluginIds = [], this._contextInfo = new plugin_context_js_1.PluginContext();
  }
  getTaskPaths() {
    const t = this.classKind === class_type_const_js_1.ClassTypeConst.HVIGOR_PROJECT ? "" : this._nodeName;
    return this._tasks.getTaskPaths().map(e => (0, task_path_util_js_1.union)(t, e));
  }
  getTaskDepends(t) {
    return this._tasks.getTaskDepends(t);
  }
  getBuildProfilePath() {
    return this._buildProfilePath;
  }
  getBuildFilePath() {
    return this._buildFilePath;
  }
  getNodeDir() {
    return this._nodePath;
  }
  getName() {
    return this._nodeName;
  }
  getPackageJsonPath() {
    return this._packageJsonPath;
  }
  bindPlugin(t) {
    return this._pluginContainer.registryPlugin(t);
  }
  bindPluginContextFunc(t, e) {
    this._contextInfo.setContextFunc(t, e), this._currentNodeLoaderPluginIds.push(t);
  }
  getPluginById(t) {
    return this._pluginContainer.getPluginById(t);
  }
  getContext(t) {
    return this._contextInfo.getContext(t);
  }
  getAllPluginIds() {
    return this._currentNodeLoaderPluginIds;
  }
  getAllTasks() {
    return this._tasks.getAllTasks();
  }
  getAllTasksCA() {
    return this._tasks.getAllTasksCA();
  }
  getTaskByName(t) {
    return this._tasks.findTask(t);
  }
  getTaskCAByName(t) {
    return this._tasks.findTaskCA(t);
  }
  task(t, e) {
    let s;
    s = undefined === e || "string" == typeof e ? {
      name: e || "default"
    } : e;
    const r = t || noop_js_1.noop,
      i = new default_internal_task_js_1.DefaultInternalTask(this, s, r);
    return this._tasks.addTask(i), i;
  }
  registry(t) {
    return this._tasks.addTask(t), t;
  }
  registerTask(t) {
    const e = new transform_hvigor_task_js_1.TransformHvigorTask(this, t);
    return e.initTaskContext(), e.initDependency(), e.initTaskRun(), this._tasks.addTask(e), t;
  }
  getTaskContext() {
    return {
      moduleName: this.getName(),
      modulePath: this.getNodeDir()
    };
  }
  hasTask(t) {
    return this._tasks.hasTask(t);
  }
  registryDependsOnTask(t, ...e) {
    this.taskMap.set(t, e), e.forEach(e => {
      t.dependsOn(e);
    });
  }
  getTaskContainer() {
    return this._tasks;
  }
  getTaskGraph() {
    return this._taskGraph;
  }
  clearTaskGraph() {
    this._taskGraph.clear();
  }
  loadConfig(t) {
    this.configOpt = t;
  }
  getConfigOpt() {
    return new loader_profile_js_1.CoreConfig(this.configOpt);
  }
  addExtraOption(t, e) {
    this.extraOption.set(t, e);
  }
  getExtraOption(t) {
    return this.extraOption.get(t);
  }
  beforeNodeEvaluate(t) {
    this.beforeNodeEvaluateFnQueue.push(t);
  }
  afterNodeEvaluate(t) {
    this.afterNodeEvaluateFnQueue.push(t);
  }
  async executeNodeHook(t) {
    let e;
    if (t === hook_const_js_1.HookType.beforeNodeEvaluate) e = this.beforeNodeEvaluateFnQueue;else {
      if (t !== hook_const_js_1.HookType.afterNodeEvaluate) return void this.logger.printErrorExit("HOOK_TYPE_ERROR", [this.getNodeDir()]);
      e = this.afterNodeEvaluateFnQueue;
    }
    for (; e.size() > 0;) {
      const s = e.pop(),
        r = hvigor_core_js_1.hvigorCore.getHvigorNodeByScriptPath(this._buildFilePath);
      if (undefined === r) return void this.logger.printErrorExit("CANNOT_FOUND_NODE_BUILD_FILE_PATH", [this._buildFilePath]);
      try {
        s && (await s.bind(this)(r));
      } catch (e) {
        if (e instanceof Error && hvigor_logger_1.ErrorUtil.isAdaptorError(e)) throw e;
        this.logger.printErrorExit("FAILED_TO_EXECUTE_NODE_HOOK", [t, e.message], [[t]], e.stack);
      }
    }
  }
  afterBindSystemPlugins(t) {
    this.afterBindSystemPluginsFnQueue.push(t);
  }
  async executeAfterBindSystemPluginsHook() {
    for (; this.afterBindSystemPluginsFnQueue.size() > 0;) {
      const t = this.afterBindSystemPluginsFnQueue.pop(),
        e = hvigor_core_js_1.hvigorCore.getHvigorNodeByScriptPath(this._buildFilePath);
      if (undefined === e) return void this.logger.printErrorExit("CANNOT_FOUND_NODE_BUILD_FILE_PATH", [this._buildFilePath]);
      t && (await t.bind(this)(e));
    }
  }
}
