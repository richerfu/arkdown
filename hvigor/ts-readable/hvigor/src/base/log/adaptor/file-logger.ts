/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/log/adaptor/file-logger.js
 */
var __decorate = this && this.__decorate || function (e, r, o, t) {
  var n,
    l = arguments.length,
    i = l < 3 ? r : null === t ? t = Object.getOwnPropertyDescriptor(r, o) : t;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) i = Reflect.decorate(e, r, o, t);else for (var a = e.length - 1; a >= 0; a--) (n = e[a]) && (i = (l < 3 ? n(i) : l > 3 ? n(r, o, i) : n(r, o)) || i);
  return l > 3 && i && Object.defineProperty(r, o, i), i;
};
function beforeAnonymizeBundleName(e, r, o) {
  const t = o.value;
  return o.value = function (...e) {
    const r = messageBundleNameAnonymize(e);
    return t.apply(this, r);
  }, o;
}
function messageBundleNameAnonymize(e) {
  if ("object" != typeof e) return e;
  if (Array.isArray(e)) return e.map((r, o) => "object" == typeof r ? messageBundleNameAnonymize(r) : e[o]);
  if ("object" == typeof e) {
    const r = {};
    return Object.keys(e).forEach(o => {
      if ("bundleName" === o && "string" == typeof e[o]) {
        const t = e[o];
        r[o] = t ? `${t[0]}***${t[t.length - 1]}` : "*****";
      } else "object" == typeof e[o] ? r[o] = messageBundleNameAnonymize(e[o]) : r[o] = e[o];
    }), r;
  }
  return e;
}
export function replaceBundleName(e, r, o) {
  if (!(null == r ? undefined : r.length)) return e;
  if (o || (o = new RegExp(r, "ig")), "string" == typeof e && o.test(e)) return e.replace(o, e => `${e[0]}***${e[e.length - 1]}`);
  if (Array.isArray(e)) return e.map(e => replaceBundleName(e, r, o));
  if ("object" == typeof e) {
    return Object.keys(e).reduce((t, n) => ({
      ...t,
      [n]: replaceBundleName(e[n], r, o)
    }), {});
  }
  return e;
}
export class FileLogger {
  constructor(e) {
    this.fileLogger = e;
  }
  debug(e, ...r) {
    return this.fileLogger.debug(e, ...r), [e, ...r];
  }
  log(e, ...r) {
    this.fileLogger.log(e, ...r);
  }
  warn(e, ...r) {
    this.fileLogger.warn(e, ...r);
  }
  info(e, ...r) {
    this.fileLogger.info(e, ...r);
  }
  error(e, ...r) {
    this.fileLogger.error(e, ...r);
  }
}
__decorate([beforeAnonymizeBundleName], FileLogger.prototype, "debug", null), __decorate([beforeAnonymizeBundleName], FileLogger.prototype, "log", null), __decorate([beforeAnonymizeBundleName], FileLogger.prototype, "warn", null), __decorate([beforeAnonymizeBundleName], FileLogger.prototype, "info", null), __decorate([beforeAnonymizeBundleName], FileLogger.prototype, "error", null);
