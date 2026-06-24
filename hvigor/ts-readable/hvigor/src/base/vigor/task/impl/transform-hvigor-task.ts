/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/task/impl/transform-hvigor-task.js
 */
import * as class_type_const_js_1 from "../../../common/options/class-type-const";
import * as core_task_js_1 from "../../../external/task/core-task";
import * as incremental_task_js_1 from "../../../external/task/incremental-task";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as input_output_js_1 from "../interface/input-output";
import * as match_task_with_node_js_1 from "../../../../common/util/match-task-with-node";
export class TransformHvigorTask extends incremental_task_js_1.IncrementalTask {
  constructor(t, e) {
    super(t, {
      name: e.name,
      group: core_task_js_1.HvigorTaskGroupType.CUSTOMIZE_TASK_GROUP,
      sourcePluginId: "custom-plugin"
    }), this.logger = hvigor_log_js_1.HvigorLogger.getLogger(TransformHvigorTask.name), this.task = e, this._inputs = super.declareInputs(), this._inputFiles = super.declareInputFiles(), this._outputFiles = super.declareOutputFiles(), this.taskRunContext = this.getTaskParam();
  }
  initTaskContext() {
    var t, e, s;
    this.taskContext = {
      ...this.task,
      context: null === (t = this.task.context) || undefined === t ? undefined : t.apply()
    }, null === (e = this.task.input) || undefined === e || e.apply(this.taskContext, [new input_output_js_1.TaskInput(this._inputs, this._inputFiles)]), null === (s = this.task.output) || undefined === s || s.apply(this.taskContext, [new input_output_js_1.TaskOutput(this._outputFiles)]);
  }
  initDependency() {
    const t = `${this.taskRunContext.moduleName}:${this.task.name}`;
    if (this.task.dependencies && Array.isArray(this.task.dependencies)) for (const e of this.task.dependencies) {
      this.logger.debug(`hvigorfile, registering dependency, taskName=${t}, dependency=${e}`);
      const s = (0, match_task_with_node_js_1.matchTaskWithNode)(e);
      s ? this.dependsOn(s.taskName, s.node) : this.dependsOn(e);
    }
    if (this.task.postDependencies && Array.isArray(this.task.postDependencies)) for (const e of this.task.postDependencies) this.logger.debug(`hvigorfile, registering postDependency, taskName=${t},\n         postDependency=${e}`), this.initPostDependencyTask(e, t);
  }
  initPostDependencyTask(t, e) {
    let s = this.node,
      n = t;
    const o = (0, match_task_with_node_js_1.matchTaskWithNode)(t);
    if (o) {
      const i = this.node.findModuleByName(o.node);
      if (!i) return void this.logger.warn(`postDependency "${t}" of task "${e}" not found.\n          Task "${e}" may not be executed due to lack of postDependencies.\n          If any of tasks in postDependencies is from a cross module node, make sure registerTask() is wrapped in the hvigor.nodesEvaluated() hook.`);
      s = i, n = o.taskName;
    }
    const i = s.getTaskCAByName(n);
    if (i) this.logger.debug("hvigorfile, registering postDependency through lazy task creationAction"), s.getName() === class_type_const_js_1.ClassTypeConst.HVIGOR_PROJECT ? i.dependsDefinedByUser.push(`:${this.task.name}`) : i.dependsDefinedByUser.push(`${this.node.getName()}:${this.task.name}`);else {
      const o = s.getTaskByName(n);
      if (!o) return void this.logger.warn(`postDependency "${t}" of task "${e}" not found.\n          Task "${e}" may not be executed due to lack of postDependencies.`);
      o.dependsOn(this.task.name, this.node);
    }
  }
  initTaskRun() {
    this.task.beforeRun && "function" == typeof this.task.beforeRun && super.beforeRun(async () => {
      var t;
      const e = null === (t = this.durationEvent) || undefined === t ? undefined : t.createSubEvent('"beforeRun" hook function', "").start();
      await this.task.beforeRun.apply(this.taskContext, [this.taskRunContext]), null == e || e.stop().setLog();
    }), this.task.afterRun && "function" == typeof this.task.afterRun && super.afterRun(async () => {
      var t;
      const e = null === (t = this.durationEvent) || undefined === t ? undefined : t.createSubEvent('"afterRun" hook function', "").start();
      await this.task.afterRun.apply(this.taskContext, [this.taskRunContext]), null == e || e.stop().setLog();
    });
  }
  getAction() {
    return this.doTaskAction;
  }
  async doTaskAction() {
    if (this.task.run && "function" == typeof this.task.run) {
      const t = hvigor_log_js_1.HvigorLogger.getLogger("no-pattern-info"),
        e = hvigor_log_js_1.HvigorLogger.redirectConsole(t);
      await this.task.run.apply(this.taskContext, [this.taskRunContext]), hvigor_log_js_1.HvigorLogger.restoreConsole(e);
    }
  }
  getTaskParam() {
    return {
      moduleName: this.node.getName(),
      modulePath: this.node.getNodeDir()
    };
  }
  declareInputs() {
    return this._inputs;
  }
  declareInputFiles() {
    return this._inputFiles;
  }
  declareOutputFiles() {
    return this._outputFiles;
  }
}
