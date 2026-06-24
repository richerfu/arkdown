/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/magic-string-replacement/index.js
 */
import * as code_fragment_js_1 from "./code-fragment";
import * as sourcemap_builder_js_1 from "./sourcemap-builder";
export class MagicStringReplacement {
  constructor(e) {
    this.prefix = "", this.suffix = "", this.fragmentAtStartIndex = new Map(), this.fragmentAtEndIndex = new Map(), this.sourceCode = e, this.listHead = new code_fragment_js_1.CodeFragment({
      start: 0,
      end: e.length,
      fragmentSourceCode: e
    }), this.fragmentAtStartIndex.set(0, this.listHead), this.fragmentAtEndIndex.set(e.length, this.listHead);
  }
  toString() {
    let e = this.prefix,
      t = this.listHead;
    for (; t;) e += t.toString(), t = t.next;
    return e + this.suffix;
  }
  append(e) {
    this.suffix += e;
  }
  appendLeft(e, t) {
    this.splitFragment(e);
    const r = this.fragmentAtEndIndex.get(e);
    r ? r.prefix += t : this.prefix += t;
  }
  remove(e, t) {
    const [r, n] = normalizeInputRange(this.sourceCode.length, e, t);
    if (r === n) return;
    this.splitFragment(r), this.splitFragment(n);
    let s = this.fragmentAtStartIndex.get(r);
    for (; s;) s.overwrite(""), s = n > s.end ? s.next : undefined;
  }
  overwrite(e, t, r) {
    const [n, s] = normalizeInputRange(this.sourceCode.length, e, t);
    if (n === s) throw new Error("Cannot overwrite a zero-length range – use appendLeft or prependRight instead");
    this.splitFragment(n), this.splitFragment(s);
    const i = this.fragmentAtStartIndex.get(n);
    if (!i) return;
    const a = this.fragmentAtEndIndex.get(s);
    let o = i;
    for (; o && o !== a;) {
      if (o.next !== this.fragmentAtStartIndex.get(o.end)) throw new Error("Cannot overwrite across a split point");
      o = o.next, o?.overwrite("");
    }
    i.overwrite(r);
  }
  generateMap(e) {
    return new sourcemap_builder_js_1.SourceMapBuilder(this.sourceCode, this.listHead, this.prefix).generateMap();
  }
  splitFragment(e) {
    if (this.fragmentAtStartIndex.has(e) || this.fragmentAtEndIndex.has(e)) return;
    let t = this.listHead;
    for (; t && !t.isInRange(e);) t = e > t.end ? t.next : t.prev;
    if (undefined === t) return;
    if (t.isOverwritten && t.content.length > 0) throw new Error("Cannot split a chunk that has already been edited");
    const [r, n] = generateTwoSplitFragment(t, e);
    this.fragmentAtStartIndex.delete(t.start), this.fragmentAtEndIndex.delete(t.end), this.fragmentAtStartIndex.set(r.start, r), this.fragmentAtEndIndex.set(r.end, r), this.fragmentAtStartIndex.set(n.start, n), this.fragmentAtEndIndex.set(n.end, n), t.prev ? t.prev.next = r : this.listHead = r, r.prev = t.prev, r.next = n, n.prev = r, n.next = t.next, t.next && (t.next.prev = n);
  }
}
function generateTwoSplitFragment(e, t) {
  const r = t - e.start,
    n = new code_fragment_js_1.CodeFragment({
      start: e.start,
      end: t,
      fragmentSourceCode: e.sourceCode.slice(0, r)
    });
  n.prefix = e.prefix;
  const s = new code_fragment_js_1.CodeFragment({
    start: t,
    end: e.end,
    fragmentSourceCode: e.sourceCode.slice(r)
  });
  return s.suffix = e.suffix, e.isOverwritten && (n.overwrite(""), s.overwrite("")), [n, s];
}
function normalizeInputRange(e, t, r) {
  const n = t >= 0 ? t : t % e + e,
    s = r >= 0 ? r : r % e + e;
  if (n > s) throw new Error("end must be greater than start");
  if (n < 0 || s > e) throw new Error("Character is out of bounds");
  return [n, s];
}
