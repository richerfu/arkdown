/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/copy-worker.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as copy_job_js_1 from "./copy-job";
export class CopyWorker {
  static async submitCopyResourcesWork(e, o, r, t, s, i) {
    r.debug(`CopyResources startTime: ${process.hrtime.bigint()}`);
    const p = o.submit(e, path_1.resolve(__dirname, "copy-job.js", "copyByCopyOptions"), {
      workInput: t,
      callback: s,
      targetWorkers: i
    });
    return p.getState() === hvigor_1.TaskState.REJECT ? (r.debug("Worker dispatch failed, running copy resources on main thread."), await (0, copy_job_js_1.copyByCopyOptions)(t).then(() => {
      null == s || s();
    }).catch(e => {
      throw e;
    }), null) : p.getId();
  }
}
