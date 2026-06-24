/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/task-input-value-entry.js
 */
import * as hvigor_common_1 from "@ohos/hvigor-common";
export class ValueEntry {
  constructor(e, t) {
    this._name = e, this._value = t, this._valueType = typeof t;
  }
  getName() {
    return this._name;
  }
  getValue() {
    return this._value;
  }
  getValueType() {
    return this._valueType;
  }
  getHash() {
    return this._hash || (this._hash = (0, hvigor_common_1.createHash)().update(this._name).update(`${this._value}`).update(`${this._valueType}`).digest("hex")), this._hash;
  }
}
