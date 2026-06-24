/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/file-lock.js
 */
import * as fse from "fs-extra";
const lockInfos = new Map();
export function lockSync(e, t) {
  const n = normalizeOption(t);
  let o = 0;
  for (; o <= n.retries;) if (!checkSync(e)) {
    try {
      getLockSync(e, n);
      break;
    } catch (e) {}
    o++;
  }
  if (o >= n.retries) throw new Error(`The registration information of the daemon cannot be obtained. Delete ${e}.lock and try again`);
  updateLock(e, n);
}
function getLockSync(e, t) {
  const n = `${e}.lock`;
  try {
    fse.mkdirSync(n);
  } catch (o) {
    if ("EEXIST" !== o.code) throw o;
    try {
      if (!isLockStale(fse.statSync(n), t)) throw new Error(`Lock file ${n} has been held by other process.`);
      fse.removeSync(n), lockSync(e, t);
    } catch (n) {
      if ("EONENT" !== n.code) throw n;
      lockSync(e, t);
    }
  }
  const o = new Date(Date.now() + 5);
  try {
    fse.utimesSync(n, o, o);
  } catch (e) {
    throw fse.removeSync(n), e;
  }
  lockInfos.set(e, {
    mtime: o.getTime(),
    lastUpdate: Date.now(),
    option: t,
    lockPath: n,
    release: false
  });
}
function updateLock(e, t) {
  var n;
  const o = `${e}.lock`,
    c = lockInfos.get(e),
    r = () => {
      clearInterval(null == c ? undefined : c.updateTimer), lockInfos.delete(e), fse.removeSync(o);
    };
  undefined !== c && (c.updateTimer = setInterval(() => {
    if (c.release) clearInterval(c.updateTimer), fse.removeSync(o);else {
      let e = c.lastUpdate + t.stale < Date.now();
      const n = new Date(Date.now());
      try {
        if (c.mtime !== fse.statSync(o).mtime.getTime() || undefined === c.mtime) return void r();
      } catch (t) {
        ("ENOENT" === t.code || e) && r();
      }
      try {
        fse.utimesSync(o, n, n);
      } catch (n) {
        e = c.lastUpdate + t.stale < Date.now(), ("ENOENT" === n.code || e) && r();
      }
      c.mtime = n.getTime(), c.lastUpdate = Date.now();
    }
  }, t.update), null === (n = c.updateTimer) || undefined === n || n.unref());
}
export function unlockSync(e) {
  const t = `${e}.lock`,
    n = lockInfos.get(e);
  n && (lockInfos.delete(e), n.release = true, clearInterval(n.updateTimer), fse.removeSync(t));
}
export function checkSync(e, t) {
  const n = `${e}.lock`,
    o = normalizeOption(t);
  if (fse.existsSync(n)) try {
    return !isLockStale(fse.statSync(n), o);
  } catch (e) {
    if ("ENOENT" === e.code) return false;
    throw e;
  }
  return false;
}
function normalizeOption(e) {
  const t = {
    stale: 1e4,
    retries: 0,
    update: 5e3,
    ...e
  };
  return t.stale = Math.max(t.stale || 2e3), t.update = Math.max(t.update, t.stale / 2), t;
}
function isLockStale(e, t) {
  return e.mtime.getTime() + t.stale < Date.now();
}
