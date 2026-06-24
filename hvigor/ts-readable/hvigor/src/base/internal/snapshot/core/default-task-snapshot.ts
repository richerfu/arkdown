/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/core/default-task-snapshot.js
 */
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as cache_util_js_1 from "../../cache/util/cache-util";
import * as task_built_in_js_1 from "../util/task-built-in";
import * as task_input_value_entry_js_1 from "../util/task-input-value-entry";
import * as wdk_1 from "@baize/wdk";
var snapshotSerializationReplacer = task_built_in_js_1.TaskBuiltIn.snapshotSerializationReplacer;
export class DefaultTaskSnapshot {
  constructor(e) {
    this._inputs = [], this._successful = false;
    const t = e.getNode();
    this._projectName = t.getProject().getName(), this._moduleName = t.getName(), this._taskName = e.getName(), this._key = (0, cache_util_js_1.getTaskSnapCacheEntryUniqueKey)(this._taskName, t), this._executionId = `${this._key}:${Date.now()}`, e.declareInputs().forEach((e, t) => this.addInput(t, e)), this._inputFiles = e.declareInputFiles().collect(), this._outputFiles = e.declareOutputFiles().collect();
  }
  getKey() {
    return this._key;
  }
  compareTo(e) {
    return this._successful && this._projectName === e.getProjectName() && this._moduleName === e.getModuleName() && this._taskName === e.getTaskName() && this.compareInputs(e) && this.compareInputFiles(e) && this.compareOutputFiles(e);
  }
  updateFilesOptions(e) {
    this._inputFiles.forEach((t, s) => {
      var i;
      t.fileSnapShotHashValue = null === (i = e.currentFileSnapShotCacheService.get(s)) || undefined === i ? undefined : i.hashValue;
    }), this._outputFiles.forEach((t, s) => {
      var i;
      t.fileSnapShotHashValue = null === (i = e.currentFileSnapShotCacheService.get(s)) || undefined === i ? undefined : i.hashValue;
    });
  }
  updateCacheFilesOptions(e) {
    this._inputFiles.forEach((t, s) => {
      var i;
      t.fileSnapShotHashValue = null === (i = e.fileSnapShotCacheService.get(s)) || undefined === i ? undefined : i.hashValue;
    }), this._outputFiles.forEach((t, s) => {
      var i;
      t.fileSnapShotHashValue = null === (i = e.fileSnapShotCacheService.get(s)) || undefined === i ? undefined : i.hashValue;
    });
  }
  getModuleName() {
    return this._moduleName;
  }
  getTaskName() {
    return this._taskName;
  }
  getProjectName() {
    return this._projectName;
  }
  isSuccessful() {
    return this._successful;
  }
  setExecutedSuccessful() {
    this._successful = true;
  }
  addInput(e, t) {
    const s = this._inputs.findIndex(t => t.getName() === e);
    s < 0 || this._inputs.splice(s, 1), this._inputs.push(new task_input_value_entry_js_1.ValueEntry(e, t));
  }
  getInputs() {
    return this._inputs;
  }
  getExecutionId() {
    return this._executionId;
  }
  compareInputs(e) {
    const t = e.getInputs();
    if (this._inputs.length !== t.length) return false;
    const s = (e, t) => e.getName() > t.getName() ? 1 : t.getName() > e.getName() ? -1 : 0;
    let i = (0, hvigor_common_1.createHash)();
    this._inputs.sort(s).forEach(e => i.update(e.getHash()));
    const a = i.digest("hex");
    i = (0, hvigor_common_1.createHash)(), e.getInputs().sort(s).forEach(e => i.update(e.getHash()));
    return a === i.digest("hex");
  }
  compareInputFiles(e) {
    return DefaultTaskSnapshot.compareFiles(this._inputFiles, e.getInputFiles());
  }
  compareOutputFiles(e) {
    return DefaultTaskSnapshot.compareFiles(this._outputFiles, e.getOutputFiles());
  }
  static compareFiles(e, t) {
    if (e.size !== t.size) return false;
    for (const [s, i] of e) {
      if (!t.has(s)) return false;
      const e = t.get(s);
      if (!(0, wdk_1.isEqual)(i, e)) return false;
    }
    return true;
  }
  getInputFiles() {
    return this._inputFiles;
  }
  getOutputFiles() {
    return this._outputFiles;
  }
  stringify() {
    return JSON.stringify(this, snapshotSerializationReplacer);
  }
}
