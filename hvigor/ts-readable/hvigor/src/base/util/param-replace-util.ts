/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/param-replace-util.js
 */
import * as hvigor_log_js_1 from "../log/hvigor-log";
export class ReplacePlaceHolderUtil {
  constructor() {
    this._log = hvigor_log_js_1.HvigorLogger.getLogger("param-replace");
  }
  static getInstance() {
    return this.instance || (this.instance = new ReplacePlaceHolderUtil()), this.instance;
  }
  replacePlaceholders(e, r) {
    if (Array.isArray(e)) return e.map(e => this.replacePlaceholders(e, r));
    if ("object" == typeof e && null !== e) {
      const t = {
        ...e
      };
      t.sourceRoots && Array.isArray(t.sourceRoots) && (t.sourceRoots = this.replacePathArray(t.sourceRoots, r)), t.directories && Array.isArray(t.directories) && (t.directories = this.replacePathArray(t.directories, r)), t.buildProfileFields && "object" == typeof t.buildProfileFields && (t.buildProfileFields = this.replaceBuildProfileFields(t.buildProfileFields, r));
      for (const [e, l] of Object.entries(t)) "sourceRoots" !== e && "directories" !== e && "buildProfileFields" !== e && (t[e] = this.replacePlaceholders(l, r));
      return t;
    }
    return e;
  }
  replaceBuildProfileFields(e, r) {
    const t = {
      ...e
    };
    for (const [e, l] of Object.entries(t)) "string" == typeof l && (t[e] = this.replaceStringPlaceholders(l, r));
    return t;
  }
  replacePathArray(e, r) {
    let t = [...e];
    return t = t.map(e => "string" == typeof e ? this.replaceStringPlaceholders(e, r) : e), t;
  }
  replaceStringPlaceholders(e, r) {
    return e.replace(/@param:([a-zA-Z0-9_.-]+)/g, (e, t) => {
      try {
        const l = this.getValueByPath(r, t);
        return undefined !== l ? l : e;
      } catch (r) {
        return this._log.warn(`无法解析占位符 ${e}:`, r), e;
      }
    });
  }
  getValueByPath(e, r) {
    if (!e || "object" != typeof e) return;
    const t = r.split(".");
    let l = e;
    for (const e of t) {
      if (undefined === l) return;
      l = l[e];
    }
    return l;
  }
}
