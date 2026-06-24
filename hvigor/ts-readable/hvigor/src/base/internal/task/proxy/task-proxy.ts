/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/proxy/task-proxy.js
 */
import * as fs from "fs";
import * as const_js_1 from "../../../../common/const/const";
import * as hvigor_core_js_1 from "../../../external/core/hvigor-core";
import * as incremental_exec_task_js_1 from "../../../external/task/incremental-exec-task";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as time_util_js_1 from "../../../util/time-util";
import * as project_cache_service_js_1 from "../../cache/project-cache-service";
import * as cache_util_js_1 from "../../cache/util/cache-util";
import * as snapshot_comparator_service_js_1 from "../../snapshot/service/snapshot-comparator-service";
import * as snapshot_generator_service_js_1 from "../../snapshot/service/snapshot-generator-service";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-task-proxy");
function withCache(e) {
  let t = null,
    s = false;
  return function (...a) {
    return s || (t = e.apply(this, ...a), s = true), t;
  };
}
export class TaskProxy {
  constructor(e) {
    var t;
    this.notDeclareAnyIncrementalLogic = false, this.useInputOutputCache = false, this.task = e, this.taskNode = e.getNode(), this.project = this.taskNode.getProject(), this.projectCacheService = project_cache_service_js_1.ProjectCacheService.getInstance(this.project), this.useInputOutputCache = null !== (t = hvigor_core_js_1.hvigorCore.getParameter().getPropertyInternal(const_js_1.INCREMENTAL_INPUT_OUTPUT_CACHE)) && undefined !== t && t, this.useInputOutputCache && (this.task.declareInputs = withCache(this.task.declareInputs), this.task.declareInputFiles = withCache(this.task.declareInputFiles), this.task.declareOutputFiles = withCache(this.task.declareOutputFiles)), this.snapshotGeneratorService = snapshot_generator_service_js_1.SnapshotGeneratorService.getInstance(), this.snapshotComparatorService = snapshot_comparator_service_js_1.SnapshotComparatorService.getInstance(), this.cacheTaskSnapShot = this.projectCacheService.getTaskSnapShot(e.getName(), this.taskNode), this.curInputFileSnapShotMap = new Map(), this.useInputOutputCache && (this.notDeclareAnyIncrementalLogic = 0 === this.task.declareInputs().size && 0 === this.task.declareInputFiles().collect().size && 0 === this.task.declareOutputFiles().collect().size, this.notDeclareAnyIncrementalLogic && e instanceof incremental_exec_task_js_1.IncrementalExecTask && (this.notDeclareAnyIncrementalLogic = "" === e.declareExecutionTool() && "" === e.declareExecutionCommand() && 0 === e.declareExecutionEnv().size));
  }
  getFileCompareResult(e, t) {
    const s = this.snapshotGeneratorService.generateFileSystemSnapshot(e, t),
      a = this.projectCacheService.getFileSnapShot(e);
    return a ? [s, snapshot_comparator_service_js_1.SnapshotComparatorService.isDiff(a, s)] : [s, true];
  }
  taskInputValueChanged(e) {
    var t, s, a, i;
    const r = null === (s = null === (t = this.cacheTaskSnapShot) || undefined === t ? undefined : t.getInputs()) || undefined === s ? undefined : s.find(t => t.getName() === e);
    this.curTaskSnapShot || (this.curTaskSnapShot = this.snapshotGeneratorService.generateTaskSnapshot(this.task));
    const n = null === (i = null === (a = this.curTaskSnapShot) || undefined === a ? undefined : a.getInputs()) || undefined === i ? undefined : i.find(t => t.getName() === e);
    return (null == r ? undefined : r.getValue()) !== (null == n ? undefined : n.getValue());
  }
  isTaskChange() {
    return this.curTaskSnapShot.updateCacheFilesOptions(this.projectCacheService), undefined !== this.cacheTaskSnapShot && this.cacheTaskSnapShot.isSuccessful() ? !!snapshot_comparator_service_js_1.SnapshotComparatorService.isDiff(this.cacheTaskSnapShot, this.curTaskSnapShot) && (_log.debug(`${this.task.getPath()} is not up-to-date, since the task snapshots are different.`), this.task.durationEvent.addTaskRunReason("The task snapshots are different."), true) : (_log.debug(`${this.task.getPath()} is not up-to-date, since no history record is available.`), this.task.durationEvent.addTaskRunReason("No history record is available."), true);
  }
  isTaskInputFilesChange() {
    var e;
    const t = this.task.declareInputFiles().collect();
    for (const [s, a] of t) {
      if (!fs.existsSync(s)) throw new Error(`Failed to find the incremental input file: ${s}.`);
      const t = this.getFileCompareResult(s, a);
      if (t[1]) return null === (e = this.curInputFileSnapShotMap) || undefined === e || e.set(s, t[0]), _log.debug(`${this.task.getPath()} is not up-to-date, since the input file '${s}' has been changed.`), this.task.durationEvent.addTaskRunReason(`The input file '${s}' has been changed.`), true;
    }
    return false;
  }
  isTaskOutputFilesChange() {
    const e = this.task.declareOutputFiles().collect();
    for (const [t, s] of e) {
      if (!fs.existsSync(t)) return _log.debug(`${this.task.getPath()} is not up-to-date, since the output file '${t}' does not exist.`), this.task.durationEvent.addTaskRunReason(`The output file '${t}' does not exist.`), true;
      if (this.getFileCompareResult(t, s)[1]) return _log.debug(`${this.task.getPath()} is not up-to-date, since the output file '${t}' has been changed.`), this.task.durationEvent.addTaskRunReason(`The output file '${t}' has been changed.`), true;
    }
    return false;
  }
  preExecute() {
    var e;
    const t = process.hrtime();
    if (this.useInputOutputCache && this.notDeclareAnyIncrementalLogic) {
      const e = process.hrtime(t),
        s = (0, time_util_js_1.formatTime)(e);
      return _log.debug(`${this.task.getPath()} is declared as IncrementTask, but does not implement any incremental method.`), _log.debug(`Incremental task ${this.task.getPath()} pre-execution cost: ${s}.`), false;
    }
    if (this.curTaskSnapShot = this.snapshotGeneratorService.generateTaskSnapshot(this.task), !(null === (e = this.cacheTaskSnapShot) || undefined === e ? undefined : e.isSuccessful())) return false;
    const s = this.isTaskInputFilesChange();
    if (s) {
      const e = process.hrtime(t),
        s = (0, time_util_js_1.formatTime)(e);
      return _log.debug(`Incremental task ${this.task.getPath()} pre-execution cost: ${s}.`), false;
    }
    const a = this.isTaskOutputFilesChange();
    if (a) {
      const e = process.hrtime(t),
        s = (0, time_util_js_1.formatTime)(e);
      return _log.debug(`Incremental task ${this.task.getPath()} pre-execution cost: ${s}.`), false;
    }
    const i = !this.isTaskChange() && !s && !a,
      r = process.hrtime(t),
      n = (0, time_util_js_1.formatTime)(r);
    return _log.debug(`Incremental task ${this.task.getPath()} pre-execution cost: ${n}.`), i;
  }
  async execute() {
    await this.task.execute();
  }
  postExecute() {
    const e = process.hrtime();
    if (this.task.taskExecutedStatus.isUpToDate()) return void _log.debug(`There is no need to refresh cache, since the incremental task ${this.task.getPath()} is up-to-date.`);
    this.updateTaskInputFilesSnapshot(), this.updateTaskOutputFilesSnapShot(), this.updateTaskSnapShot();
    const t = process.hrtime(e),
      s = (0, time_util_js_1.formatTime)(t);
    _log.debug(`Incremental task ${this.task.getPath()} post-execution cost:${s}.`);
  }
  updateTaskOutputFilesSnapShot() {
    this.task.declareOutputFiles().collect().forEach((e, t) => {
      _log.debug(`Update task ${this.task.getPath()} output file:${t} cache.`);
      const s = this.snapshotGeneratorService.generateFileSystemSnapshot(t, e);
      this.projectCacheService.updateFileSnapShot(t, s);
    });
  }
  updateTaskInputFilesSnapshot() {
    this.task.declareInputFiles().collect().forEach((e, t) => {
      if (this.curInputFileSnapShotMap.has(t)) _log.debug(`Update task ${this.task.getPath()} input file:${t} cache from map.`), this.projectCacheService.updateFileSnapShot(t, this.curInputFileSnapShotMap.get(t));else {
        _log.debug(`Update task ${this.task.getPath()} input file:${t} cache by regenerate.`);
        const s = this.snapshotGeneratorService.generateFileSystemSnapshot(t, e);
        this.projectCacheService.updateFileSnapShot(t, s);
      }
    });
  }
  updateTaskSnapShot() {
    const e = (0, cache_util_js_1.getTaskSnapCacheEntryUniqueKey)(this.task.getName(), this.taskNode);
    this.curTaskSnapShot || (this.curTaskSnapShot = this.snapshotGeneratorService.generateTaskSnapshot(this.task)), this.curTaskSnapShot.updateFilesOptions(this.projectCacheService), this.projectCacheService.updateTaskSnapShot(e, this.curTaskSnapShot);
  }
  getCurTaskSnapShot() {
    return this.curTaskSnapShot;
  }
}
