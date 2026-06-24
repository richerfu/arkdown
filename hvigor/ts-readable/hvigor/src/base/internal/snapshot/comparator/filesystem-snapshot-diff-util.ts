/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/comparator/filesystem-snapshot-diff-util.js
 */
import * as filesnapshot_detail_diff_js_1 from "./filesnapshot-detail-diff";
export class DiffUtil {
  static diff(e, t, d) {
    const n = DiffUtil.diffWithDetail(e, t, d);
    return 0 === n.removedNodes.length && 0 === n.addedNodes.length && 0 === n.updatedNodes.length;
  }
  static diffWithDetail(e, t, d) {
    const n = new filesnapshot_detail_diff_js_1.FileSnapshotDetailDiff(),
      a = e.walk(),
      i = t.walk();
    let o = a.next(),
      l = i.next();
    for (; !o.done && !l.done;) {
      const e = d.compare(o.value, l.value);
      e < 0 ? (n.removedNodes.push(o.value), o = a.next()) : e > 0 ? (n.addedNodes.push(l.value), l = i.next()) : (n.updatedNodes.push(l.value), o = a.next(), l = i.next());
    }
    for (; !o.done;) n.removedNodes.push(o.value), o = a.next();
    for (; !l.done;) n.addedNodes.push(l.value), l = i.next();
    return n;
  }
  static diffWithBreak(e, t, d, n) {
    const a = e.walk(),
      i = t.walk();
    let o = a.next(),
      l = i.next();
    for (; !o.done && !l.done;) {
      const e = d.compare(o.value, l.value);
      if (e < 0) {
        if (!n.removed(o.value)) return false;
        o = a.next();
      } else if (e > 0) {
        if (!n.added(l.value)) return false;
        l = i.next();
      } else {
        if (!n.updated(o.value, l.value)) return false;
        o = a.next(), l = i.next();
      }
    }
    for (; !o.done;) {
      if (!n.removed(o.value)) return false;
      o = a.next();
    }
    for (; !l.done;) {
      if (!n.added(l.value)) return false;
      l = i.next();
    }
    return true;
  }
}
