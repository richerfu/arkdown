/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/hvigor-process.js
 */
import cluster_1 from "cluster";
import events_1 from "events";
import process_1 from "process";
import * as log4js_1 from "log4js";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as const_js_1 from "../../../common/const/const";
import * as hook_const_js_1 from "../../../common/const/hook-const";
import * as hvigor_config_loader_js_1 from "../../../common/util/hvigor-config-loader";
import * as report_service_impl_js_1 from "../../common/report/report-service-impl";
import * as hvigor_trace_js_1 from "../../common/trace/hvigor-trace";
import * as trace_manager_js_1 from "../../common/trace/trace-manager";
import * as watch_config_file_js_1 from "../../daemon/cluster/watch-config-file";
import * as worker_process_event_id_js_1 from "../../daemon/cluster/worker-process-event-id";
import * as hvigor_core_js_1 from "../../external/core/hvigor-core";
import * as default_tooling_model_bean_registry_js_1 from "../../external/default-tooling-model-bean-registry";
import * as build_result_js_1 from "../../external/models/build-result";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as mark_event_js_1 from "../../metrics/event/mark-event";
import * as metric_factory_js_1 from "../../metrics/metric-factory";
import * as metric_service_js_1 from "../../metrics/metric-service";
import * as time_util_js_1 from "../../util/time-util";
import * as project_cache_service_js_1 from "../cache/project-cache-service";
import * as global_core_parameters_js_1 from "../data/global-core-parameters";
import * as global_data_js_1 from "../data/global-data";
import * as tcb_store_js_1 from "../pool/store/tcb-store";
import * as watch_worker_js_1 from "../pool/worker-pool/watch-worker";
import * as worker_pool_factory_js_1 from "../pool/worker-pool/worker-pool-factory";
import * as task_control_center_js_1 from "../task/core/task-control-center";
import * as task_directed_acyclic_graph_js_1 from "../task/core/task-directed-acyclic-graph";
import * as event_id_options_js_1 from "./event/event-id-options";
import * as hvigor_lifecycle_hook_js_1 from "./hook/hvigor-lifecycle-hook";
import * as sync_js_1 from "./sync";
import * as insert_solution_1 from "../../log/error-handler/insert-solution";
import * as anonymize_stack_1 from "../../common/trace/anonymize-stack";
import * as anonymize_project_config_1 from "../../common/trace/anonymize-project-config";
import fs_extra_1 from "fs-extra";
import * as json5_reader_1 from "../../util/json5-reader";
import path_1 from "path";
import * as path_util_1 from "../../../common/util/path-util";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-process");
class HvigorBuildProcess extends events_1 {
  constructor() {
    super(), this.hasTaskDone = false, process_1.on("message", e => {
      e.type === event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED && this.emit(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED, null == e ? undefined : e.reason);
    });
  }
  init() {
    this.once(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FINISHED, async () => {
      await hvigorProcessListenerFactory.getProcessListenerByEventName(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FINISHED)();
    }), this.once(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED, async e => {
      await hvigorProcessListenerFactory.getProcessListenerByEventName(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED)(e);
    });
  }
  startProcessMonitor() {
    global_core_parameters_js_1.coreParameter.startParams.analyze === global_core_parameters_js_1.AnalyzeMode.ADVANCED && report_service_impl_js_1.ReportServiceImpl.getInstance().startProcessMonitor();
  }
  setTaskBeginTime() {
    this.taskBeginTime = process_1.hrtime();
  }
  close() {
    this.hasTaskDone = false, _log.debug("hvigor build process will be closed."), this.removeAllListeners();
  }
  error(e, r) {
    const o = null != r ? r : _log;
    o.debug("hvigor build process will be closed with an error.");
    const t = this.getErrorMessageWithSolution(e);
    t && (e.message = t);
    const {
        anonymizeStack: s,
        stack: _
      } = this.getAnonymizedStack(e),
      a = this.getAnonymizedProjectConfig();
    hvigor_trace_js_1.hvigorTrace.traceErrorMessage({
      code: null == s ? undefined : s.getErrorCode(e.message),
      stack: _,
      projectConfig: a,
      message: ""
    });
    const i = hvigor_logger_1.HvigorErrorCommonAdapter.combineTrySuggestion(global_core_parameters_js_1.coreParameter.startParams.printStackTrace, global_core_parameters_js_1.coreParameter.startParams.logLevel === log4js_1.levels.DEBUG);
    e.name === hvigor_logger_1.AdaptorError.NAME ? o.error(e.message + i) : (null == e ? undefined : e.message) ? o.printErrorWithAdaptorErrorMessage(new hvigor_logger_1.MixAdaptor(e.message).getErrorMessage(), i) : o.error(e.message + i), e.stack && (true === global_core_parameters_js_1.coreParameter.startParams.printStackTrace ? o.error(e.stack) : (o.debug("ERROR: stacktrace =", e.stack), _log._printStackErrorToFile(e.stack))), hvigorProcess.emit(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED, e);
  }
  getErrorMessageWithSolution(e) {
    try {
      return new insert_solution_1.InsertSolution().insertSolution(e.message);
    } catch (e) {}
  }
  getAnonymizedProjectConfig() {
    try {
      let e = {};
      const r = path_1.resolve(path_util_1.PathUtil.getHvigorCacheDir(), "cache", "project-config.json");
      fs_extra_1.existsSync(r) && (e = json5_reader_1.Json5Reader.getJson5Obj(r));
      return new anonymize_project_config_1.AnonymizeProjectConfig().anonymizeProjectConfig(e);
    } catch (e) {}
    return {};
  }
  getAnonymizedStack(e) {
    try {
      const r = new anonymize_stack_1.AnonymizeStack(null == e ? undefined : e.stack),
        o = r.anonymize();
      return {
        anonymizeStack: r,
        stack: o
      };
    } catch (e) {}
    return {
      anonymizeStack: null,
      stack: ""
    };
  }
}
class HvigorProcessListenerFactory {
  getProcessListenerByEventName(e) {
    switch (e) {
      case event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FINISHED:
        return this.getFinishedEventListener();
      case event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED:
        return this.getFailedEventListener();
      default:
        return wdk_1.noop;
    }
  }
  getFailedEventListener() {
    return async e => {
      const r = global_data_js_1.globalData.cliEnv,
        o = metric_factory_js_1.MetricFactory.createDurationEvent("build finish", "", "");
      await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.buildFinished, new build_result_js_1.BuildResult(e ? "string" == typeof e ? new Error(e) : e : null, report_service_impl_js_1.ReportServiceImpl.getInstance().getReport()), o);
      const t = process_1.hrtime(hvigorProcess.taskBeginTime);
      this.historyDataRecord(hvigorProcess.taskBeginTime, mark_event_js_1.MarkEventState.FAILED);
      const s = (0, time_util_js_1.formatTime)(t);
      _log.error(`BUILD FAILED in ${s}`), process_1.exitCode = -1, wrapUpBeforeExit(r.cwd, 1e9 * t[0] + t[1]), cluster_1.isWorker && process_1.send({
        type: worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FAILED
      }), exitHvigor(process_1.exitCode);
    };
  }
  getFinishedEventListener() {
    const e = global_data_js_1.globalData.cliOpts,
      r = global_data_js_1.globalData.cliEnv,
      o = metric_factory_js_1.MetricFactory.createDurationEvent("build finish", "", "");
    return e.sync ? async () => {
      await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.buildFinished, new build_result_js_1.BuildResult(null, report_service_impl_js_1.ReportServiceImpl.getInstance().getReport()), o), (0, sync_js_1.outputPluginSyncInfo)(), cluster_1.isWorker && process_1.send({
        type: worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FINISHED
      }), exitHvigor();
    } : async () => {
      var e;
      await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.buildFinished, new build_result_js_1.BuildResult(null, report_service_impl_js_1.ReportServiceImpl.getInstance().getReport()), o);
      const t = process_1.hrtime(hvigorProcess.taskBeginTime),
        s = (0, time_util_js_1.formatTime)(t);
      let _ = mark_event_js_1.MarkEventState.SUCCESS;
      if (hvigorProcess.hasTaskDone) _log.info(`BUILD SUCCESSFUL in ${s}`);else {
        const r = global_data_js_1.globalData.cliOpts;
        _log.printErrorExit("TASK_WAS_NOT_FOUND_IN_THE_PROJECT", [r._, null === (e = hvigor_core_js_1.hvigorCore.getProject()) || undefined === e ? undefined : e.getName()]), _log.error(`BUILD FAILED in ${s}`), _ = mark_event_js_1.MarkEventState.FAILED;
      }
      this.historyDataRecord(hvigorProcess.taskBeginTime, _), wrapUpBeforeExit(r.cwd, 1e9 * t[0] + t[1]), cluster_1.isWorker && process_1.send({
        type: worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FINISHED
      }), exitHvigor();
    };
  }
  historyDataRecord(e, r) {
    if (!e) return;
    const o = global_data_js_1.globalData.cliOpts._.toString(),
      t = "clean" !== o,
      s = metric_factory_js_1.MetricFactory.createMarkEvent(o, "");
    let _;
    _ = global_core_parameters_js_1.coreParameter.startParams.daemon ? global_data_js_1.globalData.cliOpts.completeCommand : process_1.argv.slice(2).join(" ");
    const a = (0, wdk_1.cloneDeep)(global_data_js_1.globalData.cliOpts);
    this.deleteObjectProps(a), s.setCompleteCommand(`${JSON.stringify(a)};${null != _ ? _ : o}`), s.setHvigorVersion(const_js_1.CUR_HVIGOR_VERSION), s.setMarkType(mark_event_js_1.MarkEventType.HISTORY), s.setNodeVersion(process_1.version), s.setCategory(t ? mark_event_js_1.MarkEventCategory.BUILD : mark_event_js_1.MarkEventCategory.CLEAN), s.start(mark_event_js_1.MarkEventState.RUNNING, this.resolveTaskTime(e)), s.stop(r);
  }
  deleteObjectProps(e) {
    delete e.env;
  }
  resolveTaskTime(e) {
    return e && e[0] && e[1] ? 1e9 * e[0] + e[1] : 0;
  }
}
const hvigorProcessListenerFactory = new HvigorProcessListenerFactory();
function clearProcessContext() {
  hvigor_config_loader_js_1.HvigorConfigLoader.clean(), hvigor_log_js_1.HvigorLogger.clean(), watch_worker_js_1.normalWorker.reset(), metric_service_js_1.MetricService.getInstance().clear(), task_directed_acyclic_graph_js_1.projectTaskDag.clear(), task_control_center_js_1.taskControlCenter.clear(), default_tooling_model_bean_registry_js_1.defaultModelRegistry.clear(), hvigor_core_js_1.hvigorCore.reset(), hvigorProcess.close(), (0, global_data_js_1.resetStartData)();
}
function wrapUpBeforeExit(e, r) {
  const o = hvigor_core_js_1.hvigorCore.getProject();
  if (o) {
    project_cache_service_js_1.ProjectCacheService.getInstance(o).close();
  }
  if (traceEnd(r), global_core_parameters_js_1.coreParameter.startParams.analyze !== global_core_parameters_js_1.AnalyzeMode.FALSE) {
    report_service_impl_js_1.ReportServiceImpl.getInstance().report();
  }
}
function traceEnd(e) {
  hvigor_trace_js_1.hvigorTrace.traceTotalTime(e), hvigor_trace_js_1.hvigorTrace.transmitDataToManager(), trace_manager_js_1.traceManager.flush();
}
function exitHvigor(e) {
  hvigor_core_js_1.hvigorCore.writeMetaProperties(), hvigor_core_js_1.hvigorCore.cleanCache(), global_data_js_1.globalData.clean(), report_service_impl_js_1.ReportServiceImpl.getInstance().stopProcessMonitor(), setImmediate(() => {
    const e = e => {
      e && tcb_store_js_1.TcbStore.clear();
    };
    worker_pool_factory_js_1.WorkerPoolFactory.getDefaultWorkerPool().isResident() && global_core_parameters_js_1.coreParameter.startParams.daemon ? worker_pool_factory_js_1.WorkerPoolFactory.getDefaultWorkerPool().recycle().then(e) : worker_pool_factory_js_1.WorkerPoolFactory.getDefaultWorkerPool().terminate().then(e), clearProcessContext();
  }), (0, watch_config_file_js_1.setRunning)(false), global_core_parameters_js_1.coreParameter.startParams.daemon || process_1.removeAllListeners();
}
export const hvigorProcess = new HvigorBuildProcess();
