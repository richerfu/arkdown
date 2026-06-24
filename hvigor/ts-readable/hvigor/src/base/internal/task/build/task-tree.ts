/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/build/task-tree.js
 */
import * as os from "os";
import * as core_task_js_1 from "../../../external/task/core-task";
import * as default_task_js_1 from "../../../external/task/default-task";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as task_path_util_js_1 from "../util/task-path-util";
const log = hvigor_log_js_1.HvigorLogger.getLogger("no-pattern-info");
export class TaskTree extends default_task_js_1.DefaultTask {
  constructor(e) {
    super(e, {
      name: "taskTree",
      group: core_task_js_1.HvigorTaskGroupType.HELP_TASK_GROUP,
      isEnabled: true,
      sourcePluginId: "hvigor",
      description: "Displays the task dag tree of the HvigorCoreNode."
    });
  }
  registryAction() {
    return () => {
      const e = ["--------------------------------------------------------------------------", `All tasks dependencies from hvigor node '${this.node.getName()}'`, "--------------------------------------------------------------------------", ""],
        t = this.node.getTaskGraph(),
        r = [];
      t.getAllStartNodes().forEach(e => {
        const t = (0, task_path_util_js_1.getTaskNameFromTaskPath)(e),
          s = this.node.getTaskByName(t);
        s.getGroup() !== core_task_js_1.HvigorTaskGroupType.HELP_TASK_GROUP && s.getGroup() !== core_task_js_1.HvigorTaskGroupType.SYNC_TASK_GROUP && r.push(this.node.getTaskByName(t));
      });
      for (const t of r) {
        const r = t.getName();
        e.push(`:${this.node.getName()}:${r}`), this.renderTaskGraph(t.getDependsOn(), e, 0);
      }
      log.info(e.join(os.EOL));
    };
  }
  renderTaskGraph(e, t, r) {
    for (const s of e) {
      const {
        moduleName: e,
        taskName: o
      } = (0, task_path_util_js_1.parseTaskPath)(s);
      if (e !== this.node.getName()) {
        t.push(`${this.getEndNodePrefix(r)}${s}`);
        continue;
      }
      const a = this.node.getTaskByName(o);
      if (undefined === a) return void log.printErrorExit("CANNOT_FIND_TASK", [o, s]);
      0 === a.getDependsOn().length ? t.push(`${this.getEndNodePrefix(r)}${a.getName()}`) : (t.push(`${this.getExpandNodePrefix(r)}${a.getName()}`), this.renderTaskGraph(a.getDependsOn(), t, r + 1));
    }
  }
  getEndNodePrefix(e) {
    let t = "";
    for (let r = 0; r < e; r++) t += "    ";
    return `${t}\\--- :`;
  }
  getExpandNodePrefix(e) {
    let t = "";
    for (let r = 0; r < e; r++) t += "    ";
    return `${t}+--- :`;
  }
}
