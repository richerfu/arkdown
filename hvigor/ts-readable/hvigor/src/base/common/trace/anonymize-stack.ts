/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/anonymize-stack.js
 */
import path_1 from "path";
export class AnonymizeStack {
  constructor(t) {
    this.stack = t, this.pathList = ["sdk", "plugins", "tools"], this.errorCodeReg = /\d{8}[\w\s]+/g, this.undefinedReg = /TypeError: Cannot read properties of undefined \(reading '(\w+)'\)/g, this.whitelist = {
      matchReg: [this.errorCodeReg, this.undefinedReg],
      contains: []
    };
  }
  anonymize() {
    if (!this.stack) return "";
    const t = [];
    this.prepareWhitelistContains();
    const e = this.splitLines();
    for (let i = 0; i < e.length; i++) {
      const s = this.getAnonymizedString(e[i]);
      s.length > 0 ? t.push(s) : "..." !== t[t.length - 1] && t.push("...");
    }
    return t.join("\n");
  }
  getErrorCode(t) {
    try {
      const e = t.match(this.errorCodeReg);
      if (e) return e[0].slice(0, 8);
    } catch (t) {}
    return "";
  }
  getAnonymizedByMatchReg(t) {
    for (let e = 0; e < this.whitelist.matchReg.length; e++) {
      const i = t.match(this.whitelist.matchReg[e]);
      if (null != i) return i[0];
    }
    return null;
  }
  getAnonymizedByContains(t) {
    for (let e = 0; e < this.whitelist.contains.length; e++) {
      let i = this.whitelist.contains[e];
      if (t.includes(i)) {
        {
          const e = i.split(path_1.sep).slice(-2, -1);
          t = t.replace(i, `***${path_1.sep}${e}${path_1.sep}`);
        }
        try {
          const e = t.split(path_1.sep).pop(),
            i = null == e ? undefined : e.split(".")[0];
          let s = "",
            n = "";
          i.length >= 3 ? (s = i.slice(0, 2), n = i.slice(-1)) : (s = i.slice(0, 1), n = i.slice(-1));
          const r = e.replace(i, `${s}***${n}`);
          return t.replace(e, r);
        } catch (t) {}
      }
    }
    return null;
  }
  getAnonymizedString(t) {
    const e = this.getAnonymizedByMatchReg(t);
    if (e) return e;
    const i = this.getAnonymizedByContains(t);
    return i || "";
  }
  prepareWhitelistContains() {
    const t = [],
      e = this.getCurrentHome();
    if (e) {
      for (let i = 0; i < this.pathList.length; i++) t.push(e + path_1.sep + this.pathList[i] + path_1.sep);
      this.whitelist.contains = t;
    }
  }
  getCurrentHome() {
    const t = path_1.resolve(__dirname);
    let e = t.split("\\tools\\hvigor\\hvigor\\src\\base\\")[0];
    return e || (e = t.split("/tools/hvigor/hvigor/src/base/")[0]), e;
  }
  splitLines() {
    let t = [],
      e = [];
    if (!this.stack) return t;
    try {
      e = this.stack.split("\n");
    } catch (t) {}
    for (let i = 0; i < e.length; i++) {
      let s = e[i];
      "\r" !== s.trim() && "" !== s.trim() && (s.endsWith("\r") ? t.push(s.slice(0, -1)) : t.push(s));
    }
    return t;
  }
}
