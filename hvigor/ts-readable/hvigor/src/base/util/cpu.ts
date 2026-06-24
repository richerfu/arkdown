/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/cpu.js
 */
import path_1 from "path";
import * as worker_threads_1 from "worker_threads";
let worker;
export function startMonitor() {
  worker || (worker = new worker_threads_1.Worker(path_1.resolve(__dirname, "./monitor-script.js")), worker.on("message", r => {
    cpuUsage = r;
  }));
}
export async function endMonitor() {
  worker && (await worker.terminate());
}
export const cpuUsage = 0;
