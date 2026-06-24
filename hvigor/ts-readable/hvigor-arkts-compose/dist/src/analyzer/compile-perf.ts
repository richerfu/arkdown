/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/analyzer/compile-perf.js
 */
import * as constants_js_1 from "../util/constants";
import * as compile_event_js_1 from "./compile-event";
import * as compile_event_factory_js_1 from "./compile-event-factory";
import * as perf_constants_js_1 from "./perf-constants";
const reversePerfMap = new Map([["## initialize", "# BUILD"], ["## generate module graph", "# BUILD"], ["## sort and bind modules", "# BUILD"], ["## mark included statements", "# BUILD"], ["## before build end", "# BUILD"], ["## build end", "# BUILD"], ["## after build end", "# BUILD"], ["beforeBuildEnd", "# BUILD"], ["buildEnd", "# BUILD"], ["afterBuildEnd", "# BUILD"], ["beforeBuild", "## initialize"], ["buildStart", "## initialize"], ["afterBuildStart", "## initialize"], ["resolveId", "## generate module graph"], ["load", "## generate module graph"], ["resolveDynamicImport", "## generate module graph"], ["shouldTransformCachedModule", "## generate module graph"], ["transform", "## generate module graph"], ["moduleParsed", "## generate module graph"], ["beforeBuildEnd", "## before build end"], ["buildEnd", "## build end"], ["afterBuildEnd", "## after build end"]]);
export class CompilePerf {
  constructor(e) {
    this.timers = new Set(), this.perf = e;
  }
  clear() {
    return this.timers.clear(), compile_event_factory_js_1.CompileEventFactory.clear(), this;
  }
  collect(e) {
    this.timers.add(e);
  }
  transform() {
    const e = new Map(),
      t = new Map(),
      r = new Map();
    this.handleRollupPerf(e, t, r), compile_event_factory_js_1.CompileEventFactory.removeIf(e => -1 === e.getStartTime());
    for (const t of compile_event_factory_js_1.CompileEventFactory.getEvents().values()) t.getName() === perf_constants_js_1.ROLLUP_EVENT_NAME && e.forEach(e => {
      t.addChild(e.getId()), e.setParent(t.getId());
    }), this.handleHookInteriorEvent(t, r);
    this.mergeHookInteriorEvents(r.values());
    const n = new Set();
    for (const e of compile_event_factory_js_1.CompileEventFactory.getEvents().values()) this.perf !== constants_js_1.AnalyzeMode.VERBOSE && this.perf !== constants_js_1.AnalyzeMode.TRACE && e.isFromHook() || n.add(e);
    return n;
  }
  persist() {
    return true;
  }
  handleRollupPerf(e, t, r) {
    this.timers.forEach(n => {
      const o = compile_event_factory_js_1.CompileEventFactory.createEvent(n.name, n.time, n.frequency).setStartTime(n.startTime).setEndTime(n.endTime).setMemory(n.memory).setState("success");
      if (n.name.startsWith("# ")) e.set(n.name, o);else {
        if (n.name.startsWith("## ")) {
          if (t.set(n.name, o), !reversePerfMap.has(n.name)) return;
          const r = reversePerfMap.get(n.name),
            s = e.get(r);
          return void (s && (s.addChild(o.getId()), o.setParent(s.getId())));
        }
        if (n.name.startsWith("- plugin ")) {
          if (this.perf !== constants_js_1.AnalyzeMode.VERBOSE && this.perf !== constants_js_1.AnalyzeMode.TRACE) return void compile_event_factory_js_1.CompileEventFactory.removeEvent(o.getId());
          o.setType(compile_event_js_1.CONTINUAL_EVENT);
          const s = n.name.split("- "),
            a = s[s.length - 1];
          if (!reversePerfMap.has(a)) return;
          const i = reversePerfMap.get(a);
          let l = e.get(i);
          return this.connectHookEvent(o, l, r), l = t.get(i), void this.connectHookEvent(o, l, r);
        }
        if ("generate ast" === n.name || "analyze ast" === n.name) {
          const e = t.get("## generate module graph");
          return void (e && (e.addChild(o.getId()), o.setParent(e.getId())));
        }
        compile_event_factory_js_1.CompileEventFactory.removeEvent(o.getId());
      }
    });
  }
  connectHookEvent(e, t, r) {
    if (!t) return;
    t.addChild(e.getId()), e.setParent(t.getId()), updateName(e);
    const n = e.getName();
    r.has(n) || r.set(n, e);
  }
  handleHookInteriorEvent(e, t) {
    if (!e.isFromHook() || e.getParent()) return;
    e.setType(compile_event_js_1.CONTINUAL_EVENT);
    const r = e.getCategory();
    if (r && t.has(r)) {
      const n = t.get(r);
      n.addChild(e.getId()), e.setParent(n.getId());
    }
  }
  processChildEvents(e, t, r, n) {
    t.setType(compile_event_js_1.CONTINUAL_EVENT);
    const o = t.getName();
    if (r.has(o)) {
      const n = r.get(o);
      n.inCreFrequency().inCreTotalTime(t), t.getChildren().forEach(e => {
        const t = compile_event_factory_js_1.CompileEventFactory.getEventById(e);
        t && (t.setParent(n.getId()), n.addChild(e));
      }), compile_event_factory_js_1.CompileEventFactory.removeEvent(e);
    } else n.push(e), r.set(o, t.inCreFrequency().inCreTotalTime(t));
  }
  mergeHookInteriorEvents(e) {
    for (const t of e) {
      const e = new Map(),
        r = [];
      for (const n of t.getChildren()) {
        const t = compile_event_factory_js_1.CompileEventFactory.getEventById(n);
        t && this.processChildEvents(n, t, e, r);
      }
      t.setChildren(r), this.mergeHookInteriorEvents(e.values());
    }
  }
}
function updateName(e) {
  const t = e.getName(),
    r = t.indexOf("("),
    n = t.substring(r);
  e.setName(n);
}
