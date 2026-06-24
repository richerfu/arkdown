/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task-creator.js
 */
import * as creation_util_js_1 from "./manager/creation-util";
export class DefaultTaskCreator {}
export class GlobalTaskCreator extends DefaultTaskCreator {
  constructor(e, t, s) {
    super(), this.isFa = false, this.depends = [], this.dependsDefinedByUser = [], this.details = {
      name: "UninitializedTask"
    }, this.configure = () => {
      this.details = this.declareTaskDetail();
    }, this.afterConfigure = () => {
      this.depends = (0, creation_util_js_1.computeHookTaskDepends)(this.declareDepends(), this.plugin.getNeedExecTargetServiceList());
    }, this.finalConfigured = () => {
      this.dependsDefinedByUser.forEach(e => {
        this.depends.push(e);
      });
    }, this.service = e, this.plugin = t, this.isFa = undefined === s ? this.isFa : s;
  }
}
export class TargetTaskCreator extends DefaultTaskCreator {
  constructor(e, t) {
    var s, a, r;
    super(), this.isFa = false, this.isFastBuild = false, this.depends = [], this.dependsDefinedByUser = [], this.details = {
      name: ""
    }, this.configure = () => {
      this.details = (0, creation_util_js_1.computeTaskDetailsWithTarget)(this.declareTaskDetail(), this.targetService);
    }, this.afterConfigure = () => {
      this.depends = (0, creation_util_js_1.computeDepends)(this.declareDepends(), this.targetService);
    }, this.finalConfigured = () => {
      this.dependsDefinedByUser.forEach(e => {
        this.depends.push(e);
      });
    }, this.targetService = e, this.node = this.targetService.getModuleService().getNode(), this.isFa = undefined === t ? this.isFa : t, this.isFastBuild = (null === (r = null === (a = null === (s = e.getTargetData().getProduct()) || undefined === s ? undefined : s.buildOption) || undefined === a ? undefined : a.packOptions) || undefined === r ? undefined : r.fastBuildApp) || false;
  }
}
export class TaskCreatorManager {
  getTaskContainer() {
    return this._taskContainer;
  }
  constructor(e) {
    this._creators = [], this._taskContainer = e;
  }
  registry(e) {
    e.configure(), this._creators.push(e), this._taskContainer.registerTask(e);
  }
  configure() {
    this._creators.forEach(e => {
      e.afterConfigure(), e.finalConfigured();
    });
  }
}
