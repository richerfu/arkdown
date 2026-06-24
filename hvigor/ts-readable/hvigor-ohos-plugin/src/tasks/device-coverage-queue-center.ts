/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/device-coverage-queue-center.js
 */
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
export class DeviceCoverageQueueCenter {
  constructor() {
    this.queueList = [], this.isExecuting = false, this.executeCoverageType = "generateOhosTestResult", this.queueList = [];
  }
  async executeProxy(e, t) {
    return this.executeCoverageType = t || "generateOhosTestResult", this.isExecuting ? new Promise((t, s) => {
      this.queueList.push({
        params: e,
        resolve: t,
        reject: s
      });
    }) : this.innerExecuteProxy(e);
  }
  async dequeue() {
    if (!this.queueList.length) return;
    const e = this.queueList.shift();
    if (!e) return;
    const {
      resolve: t,
      reject: s,
      params: r
    } = e;
    try {
      await this.innerExecuteProxy(r), t(true);
    } catch (e) {
      s(e);
    }
  }
  async innerExecuteProxy(e) {
    try {
      this.isExecuting = true, "generateUnitTestResult" === this.executeCoverageType ? await (0, hvigor_arkts_compose_1.generateUnitTestResult)(e) : await (0, hvigor_arkts_compose_1.generateOhosTestResult)(e), this.isExecuting = false, this.dequeue();
    } catch (e) {
      throw this.isExecuting = false, this.queueList = [], e;
    }
  }
}
export const deviceCoverageQueueCenter = new DeviceCoverageQueueCenter();
