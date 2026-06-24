/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/run-ark.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as copy_worker_js_1 from "./copy-worker";
import * as job_js_1 from "./job";
export async function submitArkCompileWork(e, r, t, o, i, s, a) {
  o.level = t.getLevel(), o.projectPath = o.aceModuleRoot, o.buildId = hvigor_1.globalData.buildId, o.transformAbc = !!a;
  const n = r.submit(e, path_1.resolve(__dirname, "./job.js/run"), {
    workInput: o,
    callback: i,
    targetWorkers: s,
    useReturnVal: true,
    hasSideEffects: false,
    preludeDeps: ["@ohos/hvigor"],
    memorySensitive: true
  });
  return n.getState() === hvigor_1.TaskState.REJECT ? (t.debug("Worker dispatch failed, running ark compile on main thread."), o.isMainThread = true, o.transformAbc = a, await (0, job_js_1.run)(o).then(async e => {
    if (e instanceof Error) throw e;
    await i(e);
  })) : a && n.setCustomAction(hvigor_arkts_compose_1.TRANSFORM_ABC_WORKER_REQUEST, a), o.isFaMode || o.widgetCompile || !o.copyCodeResourceEnable || (await copy_worker_js_1.CopyWorker.submitCopyResourcesWork(e, r, t, o, () => {
    t.debug(`CopyResources is end, endTime: ${process.hrtime.bigint()}`);
  })), n.getState() === hvigor_1.TaskState.REJECT ? null : n.getId();
}
export async function submitArkLintWork(e, r, t, o, i, s) {
  o.level = t.getLevel(), o.projectPath = o.aceModuleRoot;
  const a = path_1.resolve(__dirname, "./job.js/runArkLint"),
    n = {
      workInput: {
        config: o,
        entry: i
      },
      hasSideEffects: false
    };
  if (r.submit(e, a, n).getState() === hvigor_1.TaskState.REJECT) {
    const e = "perform ark linter",
      r = s.createSubEvent(e, "");
    r.start(), t.debug("Worker dispatch failed, running ark lint on main thread."), o.isMainThread = true, await (0, job_js_1.runArkLint)({
      config: o,
      entry: i
    }), r.stop(), r.setLog(e, hvigor_1.MetricLogType.INFO);
  }
}
