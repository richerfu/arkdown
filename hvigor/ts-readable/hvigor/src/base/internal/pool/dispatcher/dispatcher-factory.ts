/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/dispatcher/dispatcher-factory.js
 */
import * as dispatcher_type_js_1 from "../enum/dispatcher-type";
import * as loop_priority_dispatcher_js_1 from "./loop-priority-dispatcher";
export class DispatcherFactory {
  static getDispatcher(r) {
    if (r === dispatcher_type_js_1.DispatcherType.LOOP_PRIORITY) return new loop_priority_dispatcher_js_1.LoopPriorityDispatcher();
  }
}
