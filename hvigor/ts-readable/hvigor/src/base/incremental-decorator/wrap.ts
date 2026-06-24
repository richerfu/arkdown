/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/wrap.js
 */
import * as incremental_task_js_1 from "../external/task/incremental-task";
import * as file_set_js_1 from "../internal/snapshot/util/file-set";
import * as time_util_js_1 from "../util/time-util";
import * as strategies_js_1 from "./strategies";
import * as util_js_1 from "./util";
const isIncrementalTask = e => !!e && e instanceof incremental_task_js_1.IncrementalTask,
  collectDecoratorDecorations = e => {
    const t = new Set(),
      r = {
        inputs: new Map(),
        inputFiles: new file_set_js_1.FileSet(),
        outputFiles: new file_set_js_1.FileSet(),
        ignoreDecoratorTypes: new Set()
      };
    let n = e;
    for (; isIncrementalTask(n);) {
      ((0, util_js_1.getSelfDeclaratorContainer)(n) || []).filter(({
        declaratorType: e,
        propertyKey: t
      }) => {
        const n = r.ignoreDecoratorTypes.has(e);
        return n && util_js_1.incrementalDeclaratorLogger.debug(`Incremental declaration: '${t}' will be ignored.`), !n;
      }).forEach(n => {
        const {
          propertyKey: a,
          declaratorType: s,
          options: i
        } = n;
        if (t.has(a)) return;
        t.add(a);
        const l = e[a];
        try {
          strategies_js_1.Strategies[s](a, l, r, i);
        } catch {
          util_js_1.incrementalDeclaratorLogger.warn(`Error occurred while setting '${s}' by '${a}' in task '${e.getPath()}', will treat it as normal property.`);
        }
      }), n = Object.getPrototypeOf(n);
    }
    return r;
  },
  wrapDeclareInputs = (e, t) => {
    if (!t.size) return;
    const r = e.declareInputs.bind(e);
    e.declareInputs = function () {
      const e = r();
      for (const [r, n] of t) e.set(r, n);
      return e;
    };
  },
  concatFileSet = (e, t) => {
    for (const [r, n] of t.collect()) e.addEntry(r, n ? {
      ...n
    } : undefined);
    return e;
  },
  wrapDeclareInputFiles = (e, t) => {
    if (!t.collect().size) return;
    const r = e.declareInputFiles.bind(e);
    e.declareInputFiles = function () {
      return concatFileSet(r(), t);
    };
  },
  wrapDeclareOutputFiles = (e, t) => {
    if (!t.collect().size) return;
    const r = e.declareOutputFiles.bind(e);
    e.declareOutputFiles = function () {
      return concatFileSet(r(), t);
    };
  },
  wrapIncrementalDeclarations = e => {
    if (!(0, util_js_1.hasIncrementalDecorator)(e)) return;
    const t = process.hrtime(),
      {
        inputs: r,
        inputFiles: n,
        outputFiles: a
      } = collectDecoratorDecorations(e);
    wrapDeclareInputs(e, r), wrapDeclareInputFiles(e, n), wrapDeclareOutputFiles(e, a);
    const s = process.hrtime(t),
      i = (0, time_util_js_1.formatTime)(s);
    util_js_1.incrementalDeclaratorLogger.debug(`Task '${e.getPath()}' cost while wrapping incremental declarations: ${i}`);
  };
export { wrapIncrementalDeclarations };
