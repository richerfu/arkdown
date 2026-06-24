/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/build/tasks.js
 */
import * as os from "os";
import * as core_task_js_1 from "../../../external/task/core-task";
import * as default_task_js_1 from "../../../external/task/default-task";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
const log = hvigor_log_js_1.HvigorLogger.getLogger("no-pattern-info");
export class Tasks extends default_task_js_1.DefaultTask {
  constructor(e) {
    super(e, {
      name: "tasks",
      group: core_task_js_1.HvigorTaskGroupType.HELP_TASK_GROUP,
      isEnabled: true,
      sourcePluginId: "hvigor",
      description: "Displays the tasks of the HvigorCoreNode."
    });
  }
  registryAction() {
    return () => {
      const e = this.node.getName(),
        t = this.node.getAllTasks(),
        s = ["--------------------------------------------------------------------------", `All tasks from hvigor node '${e}'`, "--------------------------------------------------------------------------", `${this.transformInfos(t).join(os.EOL)}`];
      log.info(s.join(os.EOL));
    };
  }
  transformInfos(e) {
    const t = new Map();
    e.forEach(e => {
      const s = e.getGroup();
      t.has(s) ? t.get(s).push(e) : t.set(s, [e]);
    });
    const s = [];
    return t.forEach((e, t) => {
      s.push(`${t} tasks`), s.push("-------------");
      for (const t of e) t.getDescription() ? s.push(`${t.getName()} - ${t.getDescription()}`) : s.push(`${t.getName()}`);
      s.push("");
    }), s;
  }
}
