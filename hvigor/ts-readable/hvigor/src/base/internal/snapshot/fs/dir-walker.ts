/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/fs/dir-walker.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as event_type_js_1 from "./event-type";
import * as fs_event_js_1 from "./fs-event";
export class DirWalker {
  constructor(t, e) {
    this.stack = [], this.followLinks = t, this.maxDepth = e || 1 / 0;
  }
  walk(t) {
    let e;
    try {
      e = fs_1.lstatSync(t);
    } catch (t) {
      return;
    }
    return this.visit(t, e);
  }
  next() {
    if (this.stack.length < 1) return;
    const t = this.peekLast();
    if (!t) return;
    let e, s;
    const n = t.iterator;
    do {
      const a = n.next();
      if (a.done) return this.stack.pop(), new fs_event_js_1.FsEvent(event_type_js_1.EventType.END_DIRECTORY, t.name, t.absolutePath, t.stats);
      s = a.value;
      const r = path_1.resolve(t.absolutePath, s.name);
      let i;
      try {
        i = fs_1.lstatSync(r);
      } catch (t) {
        return;
      }
      e = this.visit(r, i);
    } while (undefined === e);
    return e;
  }
  visit(t, e) {
    if (this.stack.length >= this.maxDepth) return new fs_event_js_1.FsEvent(event_type_js_1.EventType.ENTRY, path_1.basename(t), t, e);
    if (e.isSymbolicLink()) {
      if (!this.followLinks) return new fs_event_js_1.FsEvent(event_type_js_1.EventType.ENTRY, path_1.basename(t), t, e);
      const s = fs_1.readlinkSync(t);
      if (this.wouldLoop(s)) return new fs_event_js_1.FsEvent(event_type_js_1.EventType.ENTRY, path_1.basename(t), t, e);
      t = s, e = fs_1.lstatSync(s);
    }
    if (!e.isDirectory()) return new fs_event_js_1.FsEvent(event_type_js_1.EventType.ENTRY, path_1.basename(t), t, e);
    const s = fs_1.readdirSync(t, {
        withFileTypes: true
      }),
      n = new Directory(path_1.basename(t), t, e, s[Symbol.iterator]());
    return this.stack.push(n), new fs_event_js_1.FsEvent(event_type_js_1.EventType.START_DIRECTORY, path_1.basename(t), t, e);
  }
  wouldLoop(t) {
    return -1 !== this.stack.findIndex(e => e.absolutePath === t);
  }
  peekLast() {
    const t = this.stack.length;
    if (!(t < 1)) return this.stack[t - 1];
  }
}
class Directory {
  constructor(t, e, s, n) {
    this._name = t, this._absolutePath = e, this._stats = s, this._iterator = n;
  }
  get name() {
    return this._name;
  }
  get absolutePath() {
    return this._absolutePath;
  }
  get iterator() {
    return this._iterator;
  }
  get stats() {
    return this._stats;
  }
}
