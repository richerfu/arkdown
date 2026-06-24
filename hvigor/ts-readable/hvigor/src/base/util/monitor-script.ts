/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/monitor-script.js
 */
import os_1 from "os";
import * as worker_threads_1 from "worker_threads";
function cpuAvgUsage(e, s) {
  let t = 0,
    r = 0;
  const i = os_1.cpus();
  for (const e of i) r += e.times.idle, r += e.times.irq, r += e.times.sys, r += e.times.nice, r += e.times.user, t += e.times.idle;
  return {
    idleTicks: t,
    totalTicks: r,
    cpuUsage: 1 - (t - e) / (r - s)
  };
}
let res = {
  idleTicks: 0,
  totalTicks: 0,
  cpuUsage: 0
};
setInterval(() => {
  res = cpuAvgUsage(res.idleTicks, res.totalTicks), worker_threads_1.parentPort.postMessage(res.cpuUsage);
}, 100);
