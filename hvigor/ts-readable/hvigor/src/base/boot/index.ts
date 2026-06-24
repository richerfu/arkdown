/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/boot/index.js
 */
import process_1 from "process";
import * as const_js_1 from "../../common/const/const";
import * as hook_const_js_1 from "../../common/const/hook-const";
import * as hvigor_trace_js_1 from "../common/trace/hvigor-trace";
import * as process_build_analyze_mode_js_1 from "../daemon/util/process-build-analyze-mode";
import * as hvigor_js_1 from "../external/core/hvigor";
import * as core_task_js_1 from "../external/task/core-task";
import * as global_data_js_1 from "../internal/data/global-data";
import * as build_task_graph_js_1 from "../internal/lifecycle/build-task-graph";
import * as configuration_js_1 from "../internal/lifecycle/configuration";
import * as execute_mode_factory_js_1 from "../internal/lifecycle/execute/mode/execute-mode-factory";
import * as hvigor_lifecycle_hook_js_1 from "../internal/lifecycle/hook/hvigor-lifecycle-hook";
import * as hvigor_process_js_1 from "../internal/lifecycle/hvigor-process";
import * as init_js_1 from "../internal/lifecycle/init";
import * as log_event_js_1 from "../metrics/event/log-event";
import * as metric_factory_js_1 from "../metrics/metric-factory";
import * as init_env_config_props_js_1 from "./hooks/init-env-config-props";
import * as require_hook_js_1 from "./hooks/require-hook";
const modeAlias = "mode";
export async function boot(e) {
  try {
    (0, require_hook_js_1.addExtensionHandler)([".ts", ".mjs"]);
    const o = {
      cwd: process_1.cwd(),
      configProps: null,
      version: const_js_1.CUR_HVIGOR_VERSION
    };
    global_data_js_1.globalData.init(o, e), hvigor_process_js_1.hvigorProcess.init(), (0, init_env_config_props_js_1.initEnvConfigProps)(), (0, process_build_analyze_mode_js_1.processLegacyConfig)(e), hvigor_trace_js_1.hvigorTrace.traceBuildAnalysisMode(e.analyze), hvigor_process_js_1.hvigorProcess.setTaskBeginTime(), hvigor_process_js_1.hvigorProcess.startProcessMonitor(), await start();
  } catch (e) {
    e instanceof Error && hvigor_process_js_1.hvigorProcess.error(e);
  }
}
async function start() {
  const e = "init",
    o = "create hvigor project model",
    t = "configure hvigor plugin",
    r = "build task graph",
    s = "init task execution option",
    i = metric_factory_js_1.MetricFactory.createDurationEvent(e, "Initialize and build task graph.", core_task_js_1.HvigorTaskGroupType.INIT_TASK_GROUP),
    _ = i.createSubEvent(o, "Initialize hvigor project model."),
    a = i.createSubEvent(t, "Configure hvigor plugin."),
    c = i.createSubEvent(r, "Build task graph."),
    n = i.createSubEvent(s, "Init task execution option.");
  i.start(), _.start();
  const l = global_data_js_1.globalData.cliOpts,
    g = global_data_js_1.globalData.cliEnv;
  await (0, init_js_1.init)(i), _.stop().setLog(o, log_event_js_1.MetricLogType.INFO), a.start();
  const u = await (0, configuration_js_1.configuration)(a);
  a.stop().setLog(t, log_event_js_1.MetricLogType.INFO), c.start(), (0, build_task_graph_js_1.buildTaskGraph)(u), await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.taskGraphResolved, hvigor_js_1.hvigor, i), c.stop().setLog(r, log_event_js_1.MetricLogType.INFO), n.start();
  const j = {
    toRunTasks: l._,
    isNeedSync: !!l.sync
  };
  n.stop().setLog(s, log_event_js_1.MetricLogType.INFO), i.stop().setLog(e, log_event_js_1.MetricLogType.INFO);
  const p = g.configProps.get("mode");
  await new execute_mode_factory_js_1.ExecuteModeFactory(u, j).getExecutePipeline(p).startPipeline();
}
