/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/task/core-task-impl.js
 */
import * as noop_js_1 from "../../../common/util/noop";
import * as queue_js_1 from "../../../common/util/queue";
import * as stack_js_1 from "../../../common/util/stack";
import * as class_type_const_js_1 from "../../common/options/class-type-const";
import * as worker_delegator_js_1 from "../../internal/pool/worker-pool/worker-delegator";
import * as task_execute_status_js_1 from "../../internal/task/core/task-execute-status";
import * as task_pending_promises_js_1 from "../../internal/task/core/task-pending-promises";
import * as task_path_util_js_1 from "../../internal/task/util/task-path-util";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as metric_factory_js_1 from "../../metrics/metric-factory";
import * as class_identify_util_js_1 from "../../util/class-identify-util";
import * as task_impl_js_1 from "../../vigor/task/impl/task-impl";
import * as core_task_js_1 from "./core-task";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-core-task");
export class CoreTaskImpl extends core_task_js_1.CoreTask {
  constructor(e, t) {
    var s, r;
    super(), this.dependsTask = [], this.fn = noop_js_1.noop, this.afterRunFnQueue = new queue_js_1.Queue(), this.beforeRunFnStack = new stack_js_1.Stack(), this.node = e;
    const i = {
      group: core_task_js_1.HvigorTaskGroupType.OTHER_TASK_GROUP,
      isEnabled: true,
      ...t
    };
    this.name = i.name, this.group = i.group, this.isEnabled = i.isEnabled, this.description = null !== (s = i.description) && undefined !== s ? s : "", this.sourcePluginId = null !== (r = i.sourcePluginId) && undefined !== r ? r : "", this.path = CoreTaskImpl.createTaskPath(this.node, this.name), this.taskExecutedStatus = new task_execute_status_js_1.TaskExecuteStatus(), this.pendingPromises = new task_pending_promises_js_1.TaskPendingPromises(), this.durationEvent = metric_factory_js_1.MetricFactory.createDurationEvent(this.path, this.description, this.group), this.subDurationEventMap = new Map(), this.workerDelegator = new worker_delegator_js_1.WorkerPoolDelegator(), this.taskLog = hvigor_log_js_1.HvigorLogger.getLoggerWithDurationId("hvigor-core-task", this.durationEvent.getId()), this.durationEvent.setDetail(`create ${this.path} task`);
  }
  getWorkerPool() {
    return this.workerDelegator;
  }
  async execute() {
    var e;
    await (null === (e = this.executeBeforeRun) || undefined === e ? undefined : e.bind(this)());
    const t = this.getAction();
    await t.bind(this)();
  }
  taskShouldDo() {
    return true;
  }
  onFailed(e) {}
  setDependsOn(...e) {
    return this.dependsTask = e, this;
  }
  getDependsOn() {
    return this.dependsTask;
  }
  addDependsOn(e) {
    return this.dependsTask.push(e), this;
  }
  dependsOn(e, t) {
    let s, r;
    return s = e instanceof core_task_js_1.CoreTask ? e.getName() : e, r = undefined === t ? this.node : "string" == typeof t ? this.findTargetNode(t) : t, undefined === r && this.taskLog.printErrorExit("CANNOT_FIND_NODE_SETTING_TASK", [t, this.node.getName(), this.name]), this.dependsTask.push(CoreTaskImpl.createTaskPath(r, s)), this;
  }
  findTargetNode(e) {
    let t = this.node.findModuleByName(e);
    return undefined === t && this.isProject(e) && (t = this.node.getProject()), t;
  }
  isProject(e) {
    return ":" === e || e === this.node.getProject().getName();
  }
  getAction() {
    return this.fn;
  }
  getNode() {
    return this.node;
  }
  getName() {
    return this.name;
  }
  getPath() {
    return this.path;
  }
  getTaskLog() {
    return this.taskLog;
  }
  getEnabled() {
    return this.isEnabled;
  }
  setEnabled(e) {
    return this.isEnabled = e, this;
  }
  setDescription(e) {
    return this.description = e, this;
  }
  getDescription() {
    return this.description;
  }
  getGroup() {
    return this.group;
  }
  setGroup(e) {
    return this.group = e, this;
  }
  doNotTrackState(e) {
    this.taskExecutedStatus.unTrackStateReason = e;
  }
  getSourcePluginId() {
    return this.sourcePluginId;
  }
  addSubDurationEvent(e) {
    const t = this.durationEvent.createSubEvent(this.durationEvent.getName(), this.durationEvent.getDescription());
    return this.subDurationEventMap.set(e, t), t;
  }
  getSubDurationEvent(e) {
    return this.subDurationEventMap.get(e);
  }
  static createTaskPath(e, t) {
    let s = "";
    return (0, class_identify_util_js_1.instanceOf)(e, class_type_const_js_1.ClassTypeConst.HVIGOR_PROJECT) || (s = e.getName(), "" === s && _log.printErrorExit("MODULE_NAME_IS_EMPTY_STRING")), (0, task_path_util_js_1.union)(s, t);
  }
  getHvigorTask() {
    return new task_impl_js_1.TaskImpl(this);
  }
  beforeRun(e) {
    this.beforeRunFnStack.push(e);
  }
  afterRun(e) {
    this.afterRunFnQueue.push(e);
  }
  getAfterRunQueue() {
    return this.afterRunFnQueue;
  }
  getBeforeRunStack() {
    return this.beforeRunFnStack;
  }
  async executeBeforeRun() {
    for (; !this.beforeRunFnStack.isEmpty();) {
      const e = this.beforeRunFnStack.pop();
      e && (await e.bind(this)());
    }
  }
  async executeAfterRun() {
    for (; this.afterRunFnQueue.size() > 0;) {
      const e = this.afterRunFnQueue.pop();
      e && (await e.bind(this)());
    }
  }
}
