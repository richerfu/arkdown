/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/analyzer/compile-event-factory.js
 */
import crypto_1 from "crypto";
import * as compile_event_js_1 from "./compile-event";
import * as perf_constants_js_1 from "./perf-constants";
const reserveWords = new Set([perf_constants_js_1.ROLLUP_EVENT_NAME]);
export class CompileEventFactory {
  static createEvent(e, t, o, r) {
    const n = new compile_event_js_1.CompileEvent(crypto_1.randomUUID(), e, process.pid, r ?? "Main Thread", t, o);
    return CompileEventFactory.compileEvents.set(n.getId(), n), n;
  }
  static removeIf(e) {
    CompileEventFactory.compileEvents.forEach((t, o) => {
      e(t) && CompileEventFactory.removeEvent(o);
    });
  }
  static getEvents() {
    return CompileEventFactory.compileEvents;
  }
  static removeEvent(e) {
    CompileEventFactory.compileEvents.delete(e);
  }
  static getEventById(e) {
    return CompileEventFactory.compileEvents.get(e);
  }
  static getHookEventFactory(e, t) {
    const o = `(${e}) - ${t}`;
    return CompileEventFactory.hookEventFactories.has(o) || CompileEventFactory.hookEventFactories.set(o, new HookEventFactory(o)), CompileEventFactory.hookEventFactories.get(o);
  }
  static clear() {
    CompileEventFactory.compileEvents.clear();
  }
}
CompileEventFactory.compileEvents = new Map(), CompileEventFactory.hookEventFactories = new Map();
export class HookEventFactory {
  constructor(e) {
    this.category = e;
  }
  createEvent(e) {
    if (reserveWords.has(e)) return;
    const t = new compile_event_js_1.CompileEvent(crypto_1.randomUUID(), e, process.pid, "Main Thread", 0, 0, true).setCategory(this.category);
    return CompileEventFactory.compileEvents.set(t.getId(), t), t;
  }
}
