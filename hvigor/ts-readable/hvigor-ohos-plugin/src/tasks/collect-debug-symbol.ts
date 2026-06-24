/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/collect-debug-symbol.js
 */
import os_1 from "os";
import path_1 from "path";
import process_1 from "process";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
export class CollectDebugSymbol extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    var t, s;
    super(e, task_names_js_1.TaskNames.Task.COLLECT_DEBUG_SYMBOL), this._log = ohos_logger_js_1.OhosLogger.getLogger(CollectDebugSymbol.name), this.RELEASE = "release", this.DEBUG = "debug", this._taskService = e, this.isRelease = !(null === (t = e.getBuildOption()) || undefined === t ? undefined : t.debuggable), this.shouldCollectDebugSymbol = null !== (s = hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.USE_COLLECT_DEBUG_SYMBOL)) && undefined !== s ? s : this.isRelease, this.modulePath = this.pathInfo.getModulePath(), this.debugSymbolPath = this.pathInfo.getSymbolsDir(this.modulePath);
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet();
    return fs_extra_1.existsSync(this.pathInfo.getSourceMapOriginPath(this.targetService)) && e.addEntry(this.pathInfo.getSourceMapOriginPath(this.targetService)), fs_extra_1.existsSync(this.pathInfo.getObfNameCachePath()) && e.addEntry(this.pathInfo.getObfNameCachePath()), fs_extra_1.existsSync(this.pathInfo.getIntermediatesProcessLibs()) && e.addEntry(this.pathInfo.getIntermediatesProcessLibs(), {
      isDirectory: true
    }), e;
  }
  declareOutputFiles() {
    return this.debugSymbolPath ? new hvigor_1.FileSet().addEntry(this.debugSymbolPath, {
      isDirectory: true
    }) : new hvigor_1.FileSet();
  }
  async beforeAlwaysAction() {
    var e, t;
    this.isRelease = !(null === (e = this._taskService.getBuildOption()) || undefined === e ? undefined : e.debuggable), this.shouldCollectDebugSymbol = null !== (t = hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.USE_COLLECT_DEBUG_SYMBOL)) && undefined !== t ? t : this.isRelease;
  }
  taskShouldDo() {
    return this.shouldCollectDebugSymbol;
  }
  doTaskAction() {
    fs_extra_1.ensureDirSync(this.getDestinationPath()), this.copySourceMap(), this.copyNameCache(), this.createSymbolicLinkForSoFolder();
  }
  copyNameCache() {
    const e = this.pathInfo.getObfNameCachePath(),
      t = path_1.resolve(this.getDestinationPath(), `${this.getModuleName()}-${build_directory_const_js_1.BuildArtifactConst.NAME_CACHE}`);
    try {
      fs_extra_1.existsSync(e) ? fs_extra_1.copyFileSync(e, t) : this._log.debug(`${build_directory_const_js_1.BuildArtifactConst.NAME_CACHE} for module ${this.getModuleName()} does not exists`);
    } catch (t) {
      this._log.printErrorExit("COPY_FILE_ERROR", [e]);
    }
  }
  copySourceMap() {
    let e,
      t = path_1.resolve(this.getDestinationPath(), `${this.getModuleName()}-${build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP}`);
    try {
      fs_extra_1.existsSync(this.pathInfo.getSourceMapIntermediatePath()) ? (e = this.pathInfo.getSourceMapIntermediatePath(), fs_extra_1.copyFileSync(e, t)) : (e = this.pathInfo.getSourceMapOriginPath(this.targetService), fs_extra_1.existsSync(e) ? fs_extra_1.copyFileSync(e, t) : this._log.debug(`${build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP} for module ${this.getModuleName()} does not exist`));
    } catch (t) {
      this._log.printErrorExit("COPY_FILE_ERROR", [e]);
    }
  }
  initTaskDepends() {}
  getDestinationPath() {
    const e = this.isRelease ? path_1.resolve(this.debugSymbolPath, this.RELEASE) : path_1.resolve(this.debugSymbolPath, this.DEBUG);
    return fs_extra_1.existsSync(e) || fs_extra_1.mkdirSync(e, {
      recursive: true
    }), e;
  }
  createSymbolicLinkForSoFolder() {
    const e = "win32" === process_1.platform || "Windows_NT" === os_1.type() ? "junction" : "dir",
      t = this.pathInfo.getIntermediatesProcessLibs();
    if (!fs_extra_1.existsSync(t)) return;
    const s = fs_extra_1.readdirSync(t, {
      withFileTypes: true
    }).filter(e => e.isDirectory()).map(e => e.name);
    0 === s.length && this._log.debug(`There are no cpp libraries for module ${this.getModuleName()}`), s.forEach(s => {
      const o = path_1.resolve(t, s),
        i = path_1.resolve(this.getDestinationPath(), s);
      try {
        fs_extra_1.existsSync(i) || fs_extra_1.symlinkSync(o, i, e);
      } catch (e) {
        this._log.warn(`Create symbolic link for ${o} failed. Reason: ${e}`);
      }
    });
  }
  getModuleName() {
    return this.targetService.getModuleService().getModuleModel().getName();
  }
}
