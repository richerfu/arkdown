/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/abstract-error.js
 */
import fs_1 from "fs";
import * as constant_js_1 from "./constant";
export class BaseError {
  constructor(e, t) {
    this._timestamp = new Date(), this._errorJsonPaths = e, this._matchOptions = t, this._errorInfo = this.findErrorInfo(), this._errorInfo ? (this._id = this._errorInfo.id, this._code = this._errorInfo.code, this._moreInfo = this._errorInfo.moreInfo, this._checkMessage = this._errorInfo.checkMessage, this._solutions = this._errorInfo.solutions) : this._code = constant_js_1.DEFAULT_ERROR_CODE;
  }
  isMatchSuccess() {
    return !!this._errorInfo;
  }
  findErrorInfo() {
    return this.match(this._matchOptions);
  }
  match(e) {
    if (BaseError.validFields.includes(e.field) && e.value) return "id" === e.field ? this.matchById(e.value) : this.matchByField(e);
  }
  matchByField(e) {
    const t = this.getErrorInfoJson();
    if (t) for (const s of Object.keys(t)) {
      const r = t[s];
      if ("checkMessage" === e.field) {
        if (r.checkMessage && e.value.includes(r.checkMessage)) return this.putIdIntoErrorInfo(s, t[s]);
      } else if (e.value === r[e.field]) return this.putIdIntoErrorInfo(s, t[s]);
    }
  }
  matchById(e) {
    const t = this.getErrorInfoJson();
    return this.putIdIntoErrorInfo(e, null == t ? undefined : t[e]);
  }
  putIdIntoErrorInfo(e, t) {
    if (t) return {
      ...t,
      id: e
    };
  }
  getErrorInfoJson() {
    return this._errorJsonPaths.length ? this._errorJsonPaths.reduce((e, t) => ({
      ...e,
      ...this.getJsonObj(t)
    }), {}) : undefined;
  }
  getJsonObj(e, t = "utf-8") {
    if (!fs_1.existsSync(e)) return;
    const s = fs_1.readFileSync(e, {
      encoding: t
    });
    try {
      return JSON.parse(s);
    } catch (e) {
      return;
    }
  }
  get timestamp() {
    return this._timestamp;
  }
  get errorJsonPaths() {
    return this._errorJsonPaths;
  }
  get id() {
    return this._id;
  }
  set id(e) {
    this._id = e;
  }
  get message() {
    return this._message;
  }
  set message(e) {
    this._message = e;
  }
  get solutions() {
    return this._solutions;
  }
  set solutions(e) {
    this._solutions = e;
  }
  get moreInfo() {
    return this._moreInfo;
  }
  set moreInfo(e) {
    this._moreInfo = e;
  }
  get code() {
    return this._code;
  }
  set code(e) {
    this._code = e;
  }
  get stack() {
    return this._stack;
  }
  set stack(e) {
    this._stack = e;
  }
  get checkMessage() {
    return this._checkMessage;
  }
  set checkMessage(e) {
    this._checkMessage = e;
  }
  get errorInfo() {
    return this._errorInfo;
  }
}
BaseError.validFields = ["id", "checkMessage", "code"];
