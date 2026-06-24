/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/model/work.js
 */
import * as worker_id_generator_js_1 from "./worker-id-generator";
export class Work {
  constructor(r, t, e, i, s = false, o = ["@ohos/hvigor"], h = false) {
    this.id = (0, worker_id_generator_js_1.generateId)(), this.priority = r, this.workPath = e, this.targetWorkers = t, this.inputData = i, this.sideEffects = s, this.preludeDeps = o, this.memorySensitive = h;
  }
  getId() {
    return this.id;
  }
  getPriority() {
    return this.priority;
  }
  getWorkPath() {
    return this.workPath;
  }
  getInputData() {
    return this.inputData;
  }
  hasSideEffects() {
    return this.sideEffects;
  }
  getTargetWorkers() {
    return this.targetWorkers;
  }
  setPriority(r) {
    this.priority = r;
  }
  getPreludeDeps() {
    return this.preludeDeps;
  }
  isMemorySensitive() {
    return this.memorySensitive;
  }
}
