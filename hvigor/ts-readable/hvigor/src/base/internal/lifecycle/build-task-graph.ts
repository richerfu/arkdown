/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/build-task-graph.js
 */
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as task_directed_acyclic_graph_js_1 from "../task/core/task-directed-acyclic-graph";
import * as task_path_util_js_1 from "../task/util/task-path-util";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-build-task-graph"),
  taskPathCache = new Map();
export function buildTaskGraph(a) {
  a.getSubModules().forEach(a => buildGraph(a, a.getTaskPaths(), a.getTaskGraph())), buildGraph(a, a.getTaskPaths(), a.getTaskGraph());
  const {
    hasCircle: t,
    circlePath: e
  } = task_directed_acyclic_graph_js_1.projectTaskDag.checkCircle();
  t && _log.printErrorExit("CIRCULAR_DEPENDENCY", [e.join(" -> ")]);
}
function buildGraph(a, t, e) {
  const r = a.getProject().getSubModules();
  t.forEach(t => {
    const {
      moduleName: s,
      taskName: i
    } = parseTaskPathWithCache(t);
    if (undefined !== ("" === s ? a.getProject() : r.get(s))) for (const s of a.getTaskDepends(i)) {
      const {
          moduleName: i,
          taskName: o
        } = parseTaskPathWithCache(s),
        c = "" === i ? a.getProject() : r.get(i);
      if (undefined === c) return void _log.printErrorExit("CANNOT_FIND_HVIGOR_DEPEND_NODE", [i, o]);
      c.hasTask(o) || _log.printErrorExit("CANNOT_FIND_HVIGOR_TASK", [o, i, t]), e.addEdge(t, s), task_directed_acyclic_graph_js_1.projectTaskDag.addEdge(t, s);
    } else _log.printErrorExit("CANNOT_FIND_HVIGOR_NODE", [s, t]);
  });
}
const parseTaskPathWithCache = a => {
  let t = taskPathCache.get(a);
  return undefined !== t || (t = (0, task_path_util_js_1.parseTaskPath)(a), taskPathCache.set(a, t)), t;
};
