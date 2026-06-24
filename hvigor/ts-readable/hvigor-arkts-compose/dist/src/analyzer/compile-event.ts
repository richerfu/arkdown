/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/analyzer/compile-event.js
 */
import * as compile_event_factory_js_1 from "./compile-event-factory";
export const CONTINUAL_EVENT = "continual";
export const DURATION_EVENT = "duration";
export class CompileEvent {
  constructor(t, e, i, s, r, n, a) {
    this.head = {
      id: t,
      name: e,
      description: "",
      type: "duration"
    }, this.body = {
      pid: i,
      tid: s,
      startTime: -1
    }, this.additional = {
      children: [],
      state: "created",
      totalTime: r,
      frequency: n,
      fromHook: a ?? false
    };
  }
  isFromHook() {
    return this.additional.fromHook;
  }
  start(t) {
    return this.setState("running"), this.setStartTime(t), this;
  }
  stop(t = "success", e) {
    "failed" !== this.additional.state && "success" !== this.additional.state && "warn" !== this.additional.state && (this.additional.children.forEach(e => compile_event_factory_js_1.CompileEventFactory.getEventById(e)?.stop(t)), this.body.endTime = e ?? Number(process.hrtime.bigint()), this.setState(t));
  }
  startAsyncEvent(t) {
    return this.start(t);
  }
  stopAsyncEvent(t = "success", e) {
    this.body.endTime = e ?? Number(process.hrtime.bigint()), this.setState(t);
  }
  setDescription(t) {
    return this.head.description = t, this;
  }
  setState(t) {
    return this.additional.state = t, this;
  }
  setCategory(t) {
    return this.additional.category = t, this;
  }
  getCategory() {
    return this.additional.category;
  }
  createSubEvent(t) {
    const e = compile_event_factory_js_1.CompileEventFactory.createEvent(t, 0, 0, this.getTid()).setParent(this.getId());
    return this.addChild(e.getId()), e;
  }
  setStartTime(t) {
    return this.body.startTime = t ?? Number(process.hrtime.bigint()), this;
  }
  getStartTime() {
    return this.body.startTime;
  }
  setEndTime(t) {
    return this.body.endTime = t ?? Number(process.hrtime.bigint()), this;
  }
  getEndTime() {
    return this.body.endTime;
  }
  setTotalTime(t) {
    return this.additional.totalTime = t, this;
  }
  getTotalTime() {
    return this.additional.totalTime;
  }
  setMemory(t) {
    return this.additional.memory = t, this;
  }
  getMemory() {
    return this.additional.memory;
  }
  addChild(t) {
    this.additional.children.push(t);
  }
  setChildren(t) {
    this.additional.children = t;
  }
  getUseTime() {
    return this.body.endTime ? this.body.endTime - this.body.startTime : NaN;
  }
  deleteChild(t) {
    const e = this.additional.children.indexOf(t);
    -1 !== e && (compile_event_factory_js_1.CompileEventFactory.removeEvent(t), this.additional.children.splice(e, 1));
  }
  setParent(t) {
    return this.additional.parent = t, this;
  }
  getParent() {
    return this.additional.parent;
  }
  getName() {
    return this.head.name;
  }
  getId() {
    return this.head.id;
  }
  getChildren() {
    return this.additional.children;
  }
  getTid() {
    return this.body.tid;
  }
  setTid(t) {
    return this.body.tid = t, this;
  }
  setName(t) {
    this.head.name = t;
  }
  setType(t) {
    this.head.type = t;
  }
  inCreFrequency() {
    return this.additional.frequency++, this;
  }
  inCreTotalTime(t) {
    return 0 !== t.getTotalTime() && this !== t ? this.additional.totalTime += t.getTotalTime() : undefined !== t.getEndTime() && (this.additional.totalTime += t.getEndTime() - t.getStartTime()), this;
  }
}
