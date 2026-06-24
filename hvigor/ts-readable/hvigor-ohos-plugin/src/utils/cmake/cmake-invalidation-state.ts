/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/cmake/cmake-invalidation-state.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as generate_codemodel_js_1 from "../../model/cxx/generate-codemodel";
import * as ohos_logger_js_1 from "../log/ohos-logger";
import * as cmake_util_js_1 from "./cmake-util";
const _log = ohos_logger_js_1.OhosLogger.getLogger("NativeInvalidation"),
  createInvalidationState = async (e, t, i, s, a) => {
    const r = InvalidationStateBuilder.newBuilder().addInputFiles(t).addRequiredOutputFiles(i).addOptionalOutputFiles(s).addHardOutputFiles(a).setLastFingerPrint(e),
      n = await (0, readFingerPrintFile)(e.path);
    if (!n) return r.setConfigureType(ConfigureType.HARD_CONFIGURE).setFingerPrintFileExisted(false).build();
    const l = n.map(e => e.compareToCurrent()),
      o = l.filter(e => e.type === FileChangeType.UNCHANGED),
      h = l.filter(e => e.type !== FileChangeType.UNCHANGED),
      u = (t, i = true) => computeChangedFiles(h, o, t, e.lastModified, i),
      p = u(t, false),
      d = u(i),
      g = u(s, false),
      F = u(a),
      C = p.concat(d, g),
      c = F.length > 0 ? ConfigureType.HARD_CONFIGURE : C.length > 0 ? ConfigureType.SOFT_CONFIGURE : ConfigureType.NO_CONFIGURE,
      f = n.map(e => e.compareToCurrent());
    for (const e of f) {
      const t = h.find(t => t.path === e.path);
      if (t && t.type !== e.type) return _log.debug(`File ${e.path} was modified during checks for C/C++ configuration invalidation.`), r.setFingerPrintFileExisted(true).setConfigureType(c).build();
    }
    return r.setFingerPrintFileExisted(true).addAllChangesToFingerPrintFiles(h).addAllUnchangedFingerPrintFiles(o).addAllSoftChanges(C).addAllHardChanges(F).setConfigureType(c);
  };
const computeChangedFiles = (e, t, i, s, a = true) => i.map(i => {
    const r = e.find(e => e.path === i.path);
    return r ? new ChangeFile(i.path, r.type) : t.find(e => e.path === i.path) ? new ChangeFile(i.path, FileChangeType.UNCHANGED) : a && !i.exist ? new ChangeFile(i.path, FileChangeType.DELETED) : i.lastModified >= s ? new ChangeFile(i.path, FileChangeType.LAST_MODIFIED_CHANGED) : i.exist ? new ChangeFile(i.path, FileChangeType.CREATED) : new ChangeFile(i.path, FileChangeType.UNCHANGED);
  }).filter(e => e.type !== FileChangeType.UNCHANGED),
  readFingerPrintFile = async e => {
    var t;
    if (fs_extra_1.existsSync(e)) try {
      return null === (t = (await fs_extra_1.readJSON(e)).files) || undefined === t ? undefined : t.map(e => Object.setPrototypeOf(e, FileId.prototype));
    } catch (e) {
      return;
    }
  };
export const readCMakeFileApiReply = (e, t, i, s) => {
  var a, r;
  const n = new generate_codemodel_js_1.GenerateCodemodel(e),
    l = n.getObjFromIndex("cmakeFiles"),
    o = cmake_util_js_1.CmakeUtil.parseLibraries(e, t, i);
  let h = [];
  const u = [];
  if (l) {
    const e = l.paths.build,
      t = l.paths.source;
    h = l.inputs.map(e => path_1.resolve(t, e.path)).filter(i => "cmakelists.txt" === path_1.basename(i).toLowerCase() || i.toLowerCase().startsWith(t.toLowerCase()) && !i.toLowerCase().startsWith(e.toLowerCase()) && (i.toLowerCase().endsWith(".cmake.in") || i.toLowerCase().endsWith(".cmake"))).sort();
    for (const e of o.values()) null === (a = e.getFiles()) || undefined === a || a.forEach(e => u.push(path_1.resolve(t, e)));
  }
  const p = {};
  for (const e of o.values()) p[e.id] = {
    toolchain: "toolchain",
    abi: e.getAbi(),
    artifactName: e.getArtifactName(),
    output: null === (r = e.getOutputs()) || undefined === r ? undefined : r.shift(),
    runtimeFiles: e.getRuntimeFiles()
  };
  const d = {
      buildFiles: h,
      libraries: p,
      cleanCommands: [],
      buildCommands: [],
      toolchain: {
        cCompilerExecutable: "",
        cppCompilerExecutable: ""
      },
      extra: u
    },
    g = n.getObjFromIndex("cache");
  if (g) {
    const e = g.entries.find(e => "CMAKE_LINKER" === e.name);
    if (e && undefined !== e.value) {
      const t = path_1.extname(e.value),
        i = path_1.dirname(e.value);
      d.toolchain.cCompilerExecutable = path_1.resolve(i, `clang${t}`), d.toolchain.cppCompilerExecutable = path_1.resolve(i, `clang++${t}`);
    }
  }
  return d;
};
export class InvalidationStateBuilder {
  constructor() {
    this.inputFiles = [], this.requiredOutputFiles = [], this.optionalOutputFiles = [], this.hardOutputFiles = [], this.changesToFingerPrintFiles = [], this.unchangedFingerPrintFiles = [], this.configureType = ConfigureType.SOFT_CONFIGURE, this.softConfigureReasons = [], this.hardConfigureReasons = [], this.fingerPrintFileExisted = false;
  }
  static newBuilder() {
    return new InvalidationStateBuilder();
  }
  async updateFingerPrint() {
    const e = [...this.inputFiles, ...this.requiredOutputFiles, ...this.optionalOutputFiles, ...this.hardOutputFiles].map(e => e.update()).sort();
    this.lastFingerPrint && (fs_extra_1.existsSync(this.lastFingerPrint.path) && (await fs_extra_1.rm(this.lastFingerPrint.path)), await fs_extra_1.writeJSON(this.lastFingerPrint.path, {
      files: e,
      version: 1
    }));
  }
  addAllChangesToFingerPrintFiles(e) {
    return this.changesToFingerPrintFiles.push(...e), this;
  }
  addAllUnchangedFingerPrintFiles(e) {
    return this.unchangedFingerPrintFiles.push(...e), this;
  }
  setConfigureType(e) {
    return this.configureType = e, this;
  }
  addAllSoftChanges(e) {
    return this.softConfigureReasons.push(...e), this;
  }
  addAllHardChanges(e) {
    return this.hardConfigureReasons.push(...e), this;
  }
  setFingerPrintFileExisted(e) {
    return this.fingerPrintFileExisted = e, this;
  }
  build() {
    return this;
  }
  setLastFingerPrint(e) {
    return this.lastFingerPrint = e, this;
  }
  addInputFiles(e) {
    return this.inputFiles = e, this;
  }
  getInputFile() {
    return this.inputFiles;
  }
  addRequiredOutputFiles(e) {
    return this.requiredOutputFiles.push(...e), this;
  }
  getRequiredOutputFiles() {
    return this.requiredOutputFiles;
  }
  addOptionalOutputFiles(e) {
    return this.optionalOutputFiles.push(...e), this;
  }
  getOptionalOutputFiles() {
    return this.optionalOutputFiles;
  }
  addHardOutputFiles(e) {
    return this.hardOutputFiles.push(...e), this;
  }
  getHardOutputFiles() {
    return this.hardOutputFiles;
  }
  shouldConfigureMessages() {
    const e = [],
      t = this.configureType === ConfigureType.SOFT_CONFIGURE ? "" : ", will remove stale cache CMakeCache.txt";
    return this.fingerPrintFileExisted ? this.hardConfigureReasons.length > 0 ? (e.push(`- a hard configure file changed${t}`), this.hardConfigureReasons.forEach(t => e.push(`  - ${t.path} (${changeTypeToStr(t.type)})`)), e) : this.softConfigureReasons.length > 0 ? (e.push(`- a file changed${t}`), this.softConfigureReasons.forEach(t => e.push(`  - ${t.path} (${changeTypeToStr(t.type)})`)), e) : e : (e.push(`- no fingerprint file${t}`), e);
  }
  shouldConfigure() {
    return this.configureType === ConfigureType.HARD_CONFIGURE || this.configureType === ConfigureType.SOFT_CONFIGURE;
  }
  softConfigureOkay() {
    return this.configureType === ConfigureType.SOFT_CONFIGURE;
  }
}
export class FileId {
  constructor(e, t, i, s) {
    if (this.exist = false, this.path = e, t && i) return this.size = t, this.lastModified = i, void (this.exist = undefined === s ? fs_extra_1.existsSync(e) : s);
    if (fs_extra_1.existsSync(this.path)) {
      const e = fs_extra_1.statSync(this.path);
      this.size = null != t ? t : e.size, this.lastModified = null != i ? i : e.mtimeMs, this.exist = fs_extra_1.existsSync(this.path);
    } else this.size = -1, this.lastModified = -1;
  }
  static getInstance(e, t, i, s) {
    return new FileId(e, t, i, s);
  }
  compareToCurrent() {
    const e = FileId.getInstance(this.path);
    return this.compareTo(e);
  }
  compareTo(e) {
    return this.exist && !e.exist ? new ChangeFile(this.path, FileChangeType.DELETED) : !this.exist && e.exist ? new ChangeFile(this.path, FileChangeType.CREATED) : this.lastModified !== e.lastModified ? new ChangeFile(this.path, FileChangeType.LAST_MODIFIED_CHANGED) : this.size !== e.size ? new ChangeFile(this.path, FileChangeType.LENGTH_CHANGED) : new ChangeFile(this.path, FileChangeType.UNCHANGED);
  }
  update() {
    if (!fs_extra_1.existsSync(this.path)) return this.exist = false, this;
    const e = fs_extra_1.statSync(this.path);
    return this.size = e.size, this.lastModified = e.mtimeMs, this.exist = true, this;
  }
}
export class ChangeFile {
  constructor(e, t) {
    this.path = e, this.type = t;
  }
}
export var ConfigureType, FileChangeType;
(function (e) {
  e[e.UNKNOWN_CONFIGURE_TYPE = 0] = "UNKNOWN_CONFIGURE_TYPE", e[e.NO_CONFIGURE = 1] = "NO_CONFIGURE", e[e.SOFT_CONFIGURE = 2] = "SOFT_CONFIGURE", e[e.HARD_CONFIGURE = 3] = "HARD_CONFIGURE", e[e.UNRECOGNIZED = 4] = "UNRECOGNIZED";
})(ConfigureType = ConfigureType || (ConfigureType = {})), function (e) {
  e[e.UNKNOWN = 0] = "UNKNOWN", e[e.CREATED = 1] = "CREATED", e[e.DELETED = 2] = "DELETED", e[e.LAST_MODIFIED_CHANGED = 3] = "LAST_MODIFIED_CHANGED", e[e.LENGTH_CHANGED = 4] = "LENGTH_CHANGED", e[e.UNCHANGED = -1] = "UNCHANGED";
}(FileChangeType = FileChangeType || (FileChangeType = {}));
const changeTypeToStr = e => {
  switch (e) {
    case FileChangeType.CREATED:
      return "CREATED";
    case FileChangeType.DELETED:
      return "DELETED";
    case FileChangeType.LENGTH_CHANGED:
      return "LENGTH_CHANGED";
    case FileChangeType.LAST_MODIFIED_CHANGED:
      return "LAST_MODIFIED_CHANGED";
    case FileChangeType.UNCHANGED:
      return "UNCHANGED";
    case FileChangeType.UNKNOWN:
    default:
      return "UNKNOWN";
  }
};
export { createInvalidationState, readFingerPrintFile };
