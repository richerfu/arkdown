/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/hvigor-file-util.js
 */
import * as path from "path";
import fs_extra_1 from "fs-extra";
import * as hvigor_trace_factory_js_1 from "../../base/common/trace/hvigor-trace-factory";
import * as hvigor_log_js_1 from "../../base/log/hvigor-log";
import * as json5_reader_js_1 from "../../base/util/json5-reader";
var __decorate = this && this.__decorate || function (e, t, r, i) {
  var s,
    a = arguments.length,
    l = a < 3 ? t : null === i ? i = Object.getOwnPropertyDescriptor(t, r) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) l = Reflect.decorate(e, t, r, i);else for (var n = e.length - 1; n >= 0; n--) (s = e[n]) && (l = (a < 3 ? s(l) : a > 3 ? s(t, r, l) : s(t, r)) || l);
  return a > 3 && l && Object.defineProperty(t, r, l), l;
};
export class NormalizedFile {
  constructor(e) {
    this.logger = hvigor_log_js_1.HvigorLogger.getLogger(NormalizedFile.name), this.filePath = e, Object.freeze(this);
  }
  asFileList() {
    return this._asFileList();
  }
  asFileListInternal() {
    return this._asFileList();
  }
  _asFileList() {
    if (FileUtil.isDictionaryInternal(this) && this.filePath) {
      const e = getAllFilesRecursively(this.filePath),
        t = [this];
      return e.forEach(e => {
        t.push(new NormalizedFile(e));
      }), t;
    }
    return [this];
  }
  file(e) {
    return this._file(e);
  }
  fileInternal(e) {
    return this._file(e);
  }
  _file(e) {
    try {
      if (this.filePath && FileUtil.existInternal(path.resolve(this.filePath, e))) return new NormalizedFile(path.resolve(this.filePath, e));
      throw new Error(`the path: ${FileUtil.pathResolveInternal(this.filePath, e)} is not exist`);
    } catch (e) {
      e instanceof Error && dealErrorMessage(e, this.logger);
    }
  }
  getPath() {
    return this._getPath();
  }
  getPathInternal() {
    return this._getPath();
  }
  _getPath() {
    return this.filePath;
  }
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], NormalizedFile.prototype, "asFileList", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], NormalizedFile.prototype, "file", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], NormalizedFile.prototype, "getPath", null);
export class FileUtil {
  static exist(e) {
    return this._exist(e);
  }
  static existInternal(e) {
    return this._exist(e);
  }
  static _exist(e) {
    try {
      if ("string" == typeof e) return fs_extra_1.existsSync(e);
      throw new Error("The parameter type is invalid. The function requires the string type.");
    } catch (e) {
      e instanceof Error && dealErrorMessage(e, this.logger);
    }
    return false;
  }
  static isDictionary(e) {
    return this._isDictionary(e);
  }
  static isDictionaryInternal(e) {
    return this._isDictionary(e);
  }
  static _isDictionary(e) {
    const t = "string" == typeof e ? e : e.filePath;
    try {
      if (FileUtil._exist(t)) return fs_extra_1.statSync(t).isDirectory();
      throw new Error(`the Path: ${t} is not exist`);
    } catch (e) {
      e instanceof Error && dealErrorMessage(e, this.logger);
    }
    return false;
  }
  static isFile(e) {
    return this._isFile(e);
  }
  static isFileInternal(e) {
    return this._isFile(e);
  }
  static _isFile(e) {
    const t = "string" == typeof e ? e : e.filePath;
    try {
      if (FileUtil._exist(t)) return fs_extra_1.statSync(t).isFile();
      throw new Error(`the Path: ${t} is not exist`);
    } catch (e) {
      e instanceof Error && dealErrorMessage(e, this.logger);
    }
    return false;
  }
  static ensureDirSync(e) {
    this._ensureDirSync(e);
  }
  static ensureDirSyncInternal(e) {
    this._ensureDirSync(e);
  }
  static _ensureDirSync(e) {
    FileUtil._exist(e) || fs_extra_1.ensureDirSync(e);
  }
  static ensureFileSync(e) {
    this._ensureFileSync(e);
  }
  static ensureFileSyncInternal(e) {
    this._ensureFileSync(e);
  }
  static _ensureFileSync(e) {
    FileUtil._exist(e) || fs_extra_1.ensureFileSync(e);
  }
  static readFileSync(e) {
    return this._readFileSync(e);
  }
  static readFileSyncInternal(e) {
    return this._readFileSync(e);
  }
  static _readFileSync(e) {
    const t = "string" == typeof e ? e : e.filePath;
    return checkFile(t, this.logger), fs_extra_1.readFileSync(t);
  }
  static readFile(e) {
    return this._readFile(e);
  }
  static readFileInternal(e) {
    return this._readFile(e);
  }
  static _readFile(e) {
    const t = "string" == typeof e ? e : e.filePath;
    return checkFile(t, this.logger), fs_extra_1.readFile(t);
  }
  static readJson5(e) {
    return this._readJson5(e);
  }
  static readJson5Internal(e) {
    return this._readJson5(e);
  }
  static _readJson5(e) {
    const t = "string" == typeof e ? e : e.filePath;
    return checkFile(t, this.logger), json5_reader_js_1.Json5Reader.getJson5Obj(t);
  }
  static writeFile(e, t) {
    return this._writeFile(e, t);
  }
  static writeFileInternal(e, t) {
    return this._writeFile(e, t);
  }
  static _writeFile(e, t) {
    const r = "string" == typeof e ? e : e.filePath;
    return checkFile(r, this.logger), fs_extra_1.writeFile(r, t, "utf-8");
  }
  static writeFileSync(e, t) {
    this._writeFileSync(e, t);
  }
  static writeFileSyncInternal(e, t) {
    this._writeFileSync(e, t);
  }
  static _writeFileSync(e, t) {
    const r = "string" == typeof e ? e : e.filePath;
    checkFile(r, this.logger);
    try {
      fs_extra_1.writeFileSync(r, t, "utf-8");
    } catch (e) {
      this.logger.printErrorExit("FAILED_WRITE_FILE", [e, r]);
    }
  }
  static copyFile(e, t) {
    return this._copyFile(e, t);
  }
  static copyFileInternal(e, t) {
    return this._copyFile(e, t);
  }
  static _copyFile(e, t) {
    const r = "string" == typeof e ? e : e.filePath;
    return checkFile(r, this.logger), fs_extra_1.copyFile(r, t);
  }
  static copyFileSync(e, t) {
    this._copyFileSync(e, t);
  }
  static copyFileSyncInternal(e, t) {
    this._copyFileSync(e, t);
  }
  static _copyFileSync(e, t) {
    const r = "string" == typeof e ? e : e.filePath;
    checkFile(r, this.logger);
    try {
      fs_extra_1.copyFileSync(r, t);
    } catch (e) {
      this.logger.printErrorExit("FAILED_COPY_FILE", [e, r]);
    }
  }
  static pathResolve(...e) {
    return this._pathResolve(...e);
  }
  static pathResolveInternal(...e) {
    return this._pathResolve(...e);
  }
  static _pathResolve(...e) {
    return path.resolve(...e);
  }
}
function getAllFilesRecursively(e) {
  const t = [];
  return function e(r) {
    const i = fs_extra_1.readdirSync(r);
    for (const s of i) {
      const i = path.join(r, s);
      fs_extra_1.statSync(i).isDirectory() ? (t.push(i), e(i)) : t.push(i);
    }
  }(e), t;
}
function checkFile(e, t) {
  const r = FileUtil.isFileInternal(e);
  try {
    r || t.printErrorExit("IS_NOT_A_FILE_PATH", [e]);
  } catch (e) {
    e instanceof Error && dealErrorMessage(e, t);
  }
}
function dealErrorMessage(e, t) {
  if (e.stack) {
    const r = e.stack.split("\n");
    for (const i of r) if (i.includes(" at") && !i.includes("hvigor-file-util")) {
      const r = i.substring(i.indexOf("(") + 1, i.indexOf(")"));
      t.printErrorExit("FAILED_OPERATE_FILE", [e.message, r]);
    }
  }
  t.printErrorExit("FAILED_OPERATE_FILE", [e.message, "unknown file"]);
}
FileUtil.logger = hvigor_log_js_1.HvigorLogger.getLogger(FileUtil.name), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "exist", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "isDictionary", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "isFile", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "ensureDirSync", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "ensureFileSync", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "readFileSync", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "readFile", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "readJson5", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "writeFile", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "writeFileSync", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "copyFile", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "copyFileSync", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], FileUtil, "pathResolve", null);
