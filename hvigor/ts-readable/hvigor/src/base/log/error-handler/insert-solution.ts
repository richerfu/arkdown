/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/log/error-handler/insert-solution.js
 */
import path_1 from "path";
import * as json5_reader_1 from "../../util/json5-reader";
import * as hvigor_common_1 from "@ohos/hvigor-common";
export class InsertSolution {
  constructor() {
    this.COLOR_STRING_PREFIX = "[", this.RED_KEY = "[31m", this.END_KEY = "[39m", this.errorCode = "00308018", this.errorDesc = "Unknown Error", this.errorMessage = "", this.errorCodeReg = /\d{8}[\w\s]+/g, this.specificSolutions = {
      items: [],
      unknown_error: {}
    };
  }
  insertSolution(e, r) {
    let t;
    this.errorMessage = e, r && (this.params = r);
    try {
      const e = this.findSolutionWithMoreInfo(),
        r = this.formatSolution(e);
      this.errorMessage = this.insertFormatedSolution(r), t = this.insertErrorCode();
    } catch (e) {}
    return t;
  }
  insertErrorCode() {
    return this.errorMessage.match(this.errorCodeReg) || (this.errorMessage = `[31mError Code: ${this.errorCode} ${this.errorDesc}\r\n` + this.errorMessage), this.errorMessage;
  }
  formatSolution(e) {
    if (!e || 0 === e.length) return "";
    this.params && this.handleParameters(e);
    let r = e.join("\r\n  > ");
    return r = "  > " + r, r;
  }
  handleParameters(e) {
    if (this.params) for (let r = 0; r < this.params.length; r++) {
      const t = this.params[r];
      for (let s = 0; s < e.length; s++) {
        let s = e[r];
        if (s.includes("%s")) {
          s = s.replace("%s", t);
          break;
        }
      }
    }
  }
  findSolutionWithMoreInfo() {
    let e = [];
    this.specificSolutions = json5_reader_1.Json5Reader.getJson5Obj(path_1.resolve(__dirname, "..", "..", "..", "..", "res", "specific-solutions.json5"));
    const r = this.specificSolutions.items,
      t = this.specificSolutions.unknown_error;
    for (let t = 0; t < r.length; t++) {
      const s = r[t],
        o = s.keys;
      if (this.isKeysMatch(o)) {
        e = e.concat(s.solutions), e.push(this.getMoreInfo(s.moreInfo)), this.errorCode = s.code, this.errorDesc = s.description;
        break;
      }
    }
    return 0 === e.length && t && (this.errorCode = t.code, this.errorDesc = t.description, e.push(...t.solutions), e.push(this.getMoreInfo(t.moreInfo))), e;
  }
  getMoreInfo(e) {
    return `More info: ${e[(0, hvigor_common_1.getOsLanguage)()]}`;
  }
  isKeysMatch(e) {
    if (!e) return false;
    for (let r = 0; r < e.length; r++) {
      const t = e[r];
      if (-1 === this.errorMessage.indexOf(t)) return false;
    }
    return true;
  }
  insertFormatedSolution(e) {
    if ("" === e) return this.errorMessage;
    if (this.errorMessage.indexOf("Try the following:") > -1) return this.errorMessage;
    if (this.errorMessage.match(this.errorCodeReg)) return this.errorMessage;
    const r = this.errorMessage.indexOf("Try:");
    if (r > -1) return this.errorMessage.slice(0, r) + `\r\n\r\n* Try the following:\r\n${e}\r\n` + this.errorMessage.slice(r);
    let t = this.findLastRedIndex();
    return -1 === t && (t = this.errorMessage.length), this.errorMessage.slice(0, t) + `\r\n\r\n* Try the following:\r\n${e}\r\n` + this.errorMessage.slice(t);
  }
  findLastRedIndex() {
    const e = this.findLastIndex([this.RED_KEY]),
      r = this.findLastIndex([this.END_KEY]);
    if (-1 === r) return this.errorMessage.length;
    if (e > r) return e + this.RED_KEY.length;
    const t = this.getColorIndex(e + this.RED_KEY.length, r);
    return t > -1 ? e + t + 3 : r;
  }
  getColorIndex(e, r) {
    return this.errorMessage.slice(e, r).indexOf(this.COLOR_STRING_PREFIX);
  }
  findLastIndex(e) {
    let r = -1;
    for (let t = 0; t < e.length; t++) {
      const s = e[t];
      if (r = this.errorMessage.lastIndexOf(s), -1 !== r) return r;
    }
    return r;
  }
}
