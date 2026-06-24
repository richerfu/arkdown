/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/configuration.js
 */
import cluster_1 from "cluster";
import path_1 from "path";
import process_1 from "process";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as hook_const_js_1 from "../../../common/const/hook-const";
import * as common_const_js_1 from "../../common/options/common-const";
import * as hvigor_trace_js_1 from "../../common/trace/hvigor-trace";
import * as watch_config_file_js_1 from "../../daemon/cluster/watch-config-file";
import * as hvigor_core_js_1 from "../../external/core/hvigor-core";
import * as hvigor_js_1 from "../../external/core/hvigor";
import * as hvigor_system_plugin_js_1 from "../../external/plugin/hvigor-system-plugin";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as log_event_js_1 from "../../metrics/event/log-event";
import * as file_util_js_1 from "../../util/file-util";
import * as time_util_js_1 from "../../util/time-util";
import * as global_core_parameters_js_1 from "../data/global-core-parameters";
import * as global_data_js_1 from "../data/global-data";
import * as task_tree_js_1 from "../task/build/task-tree";
import * as tasks_js_1 from "../task/build/tasks";
import * as task_prune_js_1 from "../task/prune/task-prune";
import * as ts_check_js_1 from "../util/ts_check";
import * as hvigor_lifecycle_hook_js_1 from "./hook/hvigor-lifecycle-hook";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("configuration");
export async function configuration(e) {
  const o = "init configuration",
    t = "ohpm install",
    i = "eval hook",
    r = "fin configuration",
    n = e.createSubEvent(o, "Initialize configuration."),
    s = e.createSubEvent("configure project task", "Configure project task."),
    a = e.createSubEvent("eval project", "Evaluate project."),
    _ = e.createSubEvent("eval modules", "Evaluate modules."),
    g = e.createSubEvent("add config dependencies", "Add configuration dependencies."),
    l = e.createSubEvent(t, "Ohpm install event."),
    c = e.createSubEvent(i, "EvaluateEvent hook."),
    u = e.createSubEvent(r, "Finish configuration.");
  n.start();
  const v = process_1.hrtime(),
    d = global_data_js_1.globalData.cliEnv,
    f = hvigor_core_js_1.hvigorCore.getProject(),
    p = path_1.resolve(d.cwd, common_const_js_1.HvigorCommonConst.BUILD_FILE_NAME);
  n.stop().setLog(o, log_event_js_1.MetricLogType.INFO), configProject(f, s), await evalProject(f, p, a), await evalSubModules(f, _), addConfigDependencies(g), await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.nodesEvaluated, hvigor_js_1.hvigor, e), l.start(), await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runFn(hvigor_core_js_1.hvigorCore.getHvigorNodesBeforeEvaluatedInternalHook(), hvigor_js_1.hvigor), l.stop().setLog(t, log_event_js_1.MetricLogType.INFO), c.start(), await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runFn(hvigor_core_js_1.hvigorCore.getHvigorNodesEvaluatedInternalHook(), hvigor_js_1.hvigor), c.stop().setLog(i, log_event_js_1.MetricLogType.INFO), u.start();
  const h = process_1.hrtime(v),
    j = (0, time_util_js_1.formatTime)(h);
  return _log.debug(`Configuration phase cost:${j}`), u.stop().setLog(r, log_event_js_1.MetricLogType.INFO), f;
}
function configProject(e, o) {
  o.start(), configNodeTask(e), configProjectTask(e), o.stop().setLog();
}
async function evalProject(e, o, t) {
  t.start();
  const i = hvigor_js_1.hvigor.getNodeByNameInternal(e.getName());
  undefined !== i && (await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.beforeNodeEvaluate, i, t)), await e.executeNodeHook(hook_const_js_1.HookType.beforeNodeEvaluate), await evaluateNodeVigorFile(e, (0, file_util_js_1.findRealHvigorFilePath)(o, _log), t);
  const r = hvigor_core_js_1.hvigorCore.getHvigorAfterNodeInternalHookList(e.getName());
  try {
    undefined !== i && undefined !== r && (await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runFn(r, i)), undefined !== i && (await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.afterNodeEvaluate, i, t)), await e.executeNodeHook(hook_const_js_1.HookType.afterNodeEvaluate);
  } catch (e) {
    if (hvigor_logger_1.ErrorUtil.isAdaptorError(e)) throw e;
    _log.printErrorExit("CATCH_ERROR_IN_HVIGOR_FILE", [e.name.toString().replace("hvigor_1.", ""), e.message.toString().replace("hvigor_1.", ""), `${o.toString().replace("hvigor_1.", "")}.ts`]);
  }
  t.stop().setLog();
}
async function evalSubModules(e, o) {
  o.start();
  for (const t of e.getAllSubModules()) {
    const e = "eval submodule",
      i = o.createSubEvent(e, "Evaluate submodule.");
    i.start(), configNodeTask(t);
    const r = hvigor_js_1.hvigor.getNodeByNameInternal(t.getName());
    undefined !== r && (await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.beforeNodeEvaluate, r, i)), await t.executeNodeHook(hook_const_js_1.HookType.beforeNodeEvaluate), await evaluateNodeVigorFile(t, (0, file_util_js_1.findRealHvigorFilePath)(t.getBuildFilePath(), _log), i);
    const n = hvigor_core_js_1.hvigorCore.getHvigorAfterNodeInternalHookList(t.getName());
    try {
      undefined !== r && undefined !== n && (await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runFn(n, r)), undefined !== r && (await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.afterNodeEvaluate, r, i)), await t.executeNodeHook(hook_const_js_1.HookType.afterNodeEvaluate);
    } catch (e) {
      if (hvigor_logger_1.ErrorUtil.isAdaptorError(e)) throw e;
      _log.printErrorExit("CATCH_ERROR_IN_HVIGOR_FILE", [e.name.toString().replace("hvigor_1.", ""), e.message.toString().replace("hvigor_1.", ""), path_1.resolve(null == r ? undefined : r.getNodePathInternal(), "hvigorfile.ts").toString().replace("hvigor_1.", "")]);
    }
    i.stop().setLog(e, log_event_js_1.MetricLogType.INFO);
  }
  o.stop().setLog();
}
function addConfigDependencies(e) {
  if (e.start(), cluster_1.isWorker) {
    const e = process_1.hrtime();
    (0, watch_config_file_js_1.addConfigDeps)();
    const o = process_1.hrtime(e),
      t = (0, time_util_js_1.formatTime)(o);
    _log.debug(`hvigorfile, resolve hvigorfile dependencies in ${t}`);
  }
  e.stop().setLog();
}
function configNodeTask(e) {
  e.registry(new tasks_js_1.Tasks(e)), e.registry(new task_tree_js_1.TaskTree(e));
}
function configProjectTask(e) {
  e.registry(new task_prune_js_1.PruneTask(e));
}
async function bindSystemPlugins(e, o, t) {
  if (_log.debug("hvigorfile, binding system plugins", e), "function" == typeof e) {
    const t = await e(o);
    return o.bindPlugin(t), void o.bindPluginContextFunc(t.getPluginId(), t.getContext.bind(t));
  }
  if (!(e instanceof hvigor_system_plugin_js_1.HvigorSystemPlugin)) {
    Object.keys(e).length || _log.printErrorExit("NO_SYSTEM_PLUGINS_WERE_FOUND_IN_HVIGORFILE", [t]);
    for (const i of Object.keys(e)) {
      const r = e[i];
      if (r instanceof hvigor_system_plugin_js_1.HvigorSystemPlugin) o.bindPlugin(r);else if ("function" == typeof r) {
        const e = await r(o);
        e.getPluginId || _log.printErrorExit("NO_SYSTEM_PLUGINS_WERE_FOUND_IN_HVIGORFILE", [t]), o.bindPlugin(e);
      }
    }
  }
}
function applyCustomPlugin(e, o, t, i) {
  t.apply && hvigor_core_js_1.hvigorCore.hvigorAfterNodeInternalHook(e.getName(), async () => {
    const e = "apply custom plugin",
      r = o.createSubEvent(e, `Apply custom plugin ${t.pluginId}.`);
    r.start();
    const n = Number(process_1.hrtime.bigint());
    _log.debug(`hvigorfile, executing apply function of custom plugin, pluginId = ${t.pluginId}`);
    try {
      await t.apply(i);
    } catch (e) {
      if (hvigor_logger_1.ErrorUtil.isAdaptorError(e)) throw e;
      _log.printErrorExit("FAILED_TO_EXECUTE_FUNCTION_%S_OF_THE_CUSTOM_PLUGIN", ["apply", t.pluginId, e.message], [["apply", t.pluginId]], e.stack);
    }
    r.stop().setLog(e, log_event_js_1.MetricLogType.INFO);
    const s = Number(process_1.hrtime.bigint()) - n;
    hvigor_trace_js_1.hvigorTrace.traceCustomPluginTime(t.pluginId, s);
  });
}
async function bindCustomPlugins(e, o, t, i) {
  if (_log.debug("hvigorfile, begin to bind custom plugins..."), !e || Array.isArray(e) && 0 === e.length) return void _log.debug(`hvigorfile, no custom plugins were found in ${t}`);
  if (!Array.isArray(e)) return void _log.warn(`hvigorfile, invalid custom plugins config found in ${t}`);
  const r = hvigor_core_js_1.hvigorCore.getHvigorNodeByScriptPath(t.substring(0, t.lastIndexOf(".")));
  for (let n = 0; n < e.length; n++) {
    const s = e[n];
    "object" == typeof s && "function" == typeof s.apply ? ("string" != typeof s.pluginId && _log.warn(`The plugin 'plugins[${n}]' in the file '${t}' must contain a 'pluginId' with string type `), s.context && o.bindPluginContextFunc(s.pluginId, s.context.bind(s)), applyCustomPlugin(o, i, s, r)) : _log.warn(`The plugin 'plugins[${n}]' in the file '${t}' is invalid. This plugin will not be loaded. A valid plugin must contain a 'pluginId' with string type and an 'apply' function`);
  }
}
async function evaluateNodeVigorFile(e, o, t) {
  var i;
  const r = "eval hvigorfile",
    n = "require hvigorfile",
    s = "bind plugins",
    a = t.createSubEvent(r, "Evaluate hvigorfile."),
    _ = a.createSubEvent(n, "Require hvigorfile."),
    g = a.createSubEvent(s, "Bind plugins.");
  let l;
  a.start(), _.start(), _log.debug(`hvigorfile, resolving ${o}`), global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck && o.endsWith(".ts") && ts_check_js_1.hvigorfileTSRoots.add(o), delete require.cache[require.resolve(o)];
  try {
    l = require(o), _log.debug("hvigorfile, require result: ", l);
  } catch (e) {
    e instanceof ReferenceError && "TransformStream is not defined" === e.message && _log.printErrorExit("NODE_VERSION_TOO_LOW", [e.message], undefined, e.stack), (null === (i = e.message) || undefined === i ? undefined : i.includes("Cannot find module")) && _log.printErrorExit("HVIGORFILE_CANNOT_FIND_MODULE", [e.message, o], undefined, e.stack), _log.printErrorExit("FAILED_TO_REQUIRE_HVIGORFILE", [e.message, o], undefined, e.stack);
  }
  _.stop().setLog(n, log_event_js_1.MetricLogType.INFO), g.start();
  const c = l.default;
  c && c.system ? (parseConfig(e, l.default.config), await bindSystemPlugins(l.default.system, e, o), await e.executeAfterBindSystemPluginsHook(), await bindCustomPlugins(l.default.plugins, e, o, g)) : (await bindSystemPlugins(l, e, o), await e.executeAfterBindSystemPluginsHook()), _log.debug(`hvigorfile, resolve finished ${o}`), cluster_1.isWorker && (0, watch_config_file_js_1.addModuleDependency)(o), g.stop().setLog(s, log_event_js_1.MetricLogType.INFO), a.stop().setLog(r, log_event_js_1.MetricLogType.INFO);
}
function parseConfig(e, o) {
  o && e.loadConfig(o);
}
