/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/native-cmake-resolver.js
 */
import fs_extra_1 from "fs-extra";
import * as glob_1 from "glob";
import os_1 from "os";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as cpu_abi_enum_js_1 from "../enum/cpu-abi-enum";
import * as ohos_logger_js_1 from "./log/ohos-logger";
import * as cmake_util_js_1 from "./cmake/cmake-util";
import * as hvigor_1 from "@ohos/hvigor";
import * as minimatch_1 from "minimatch";
const log = ohos_logger_js_1.OhosLogger.getLogger("ConfigureCmake");
export class PackageResolver {
  constructor(e, t, i, s) {
    var a, r, n;
    this.archs = [], this.linkLibrariesMap = new Map(), this.canonicalName = e.getPackageName(), this.name = this.canonicalName.replace(common_const_js_1.NativeConst.PACKAGE_NAME_REGEX, ""), this.version = e.getDependencyVersion(), this.abiList = t, this.path = e.getDependencyRootPath(), this.cmakeDir = i, this.service = s, this.headerPath = this.getHeaderPath(e, s), this.libsPath = s ? s.getTargetData().getPathInfo().getIntermediatesStrippedLibsDir() : path_1.resolve(e.getDependencyRootPath(), "libs"), this.matchRule = (null !== (n = null === (r = null === (a = null == s ? undefined : s.getBuildOption().nativeLib) || undefined === a ? undefined : a.filter) || undefined === r ? undefined : r.select) && undefined !== n ? n : []).find(e => e.package === this.canonicalName && e.version === this.version), this.addLibrariesInfo(e.getPackageFilePath(), e.isLocal()), this.archs = this.abiList.map(e => ArchResolver.getInstance(e, this));
  }
  static getInstance(e, t, i, s) {
    return new PackageResolver(e, t, i, s);
  }
  async build() {
    const e = [];
    for (let t of this.archs) e.push(await t.build());
    return e;
  }
  addLibrariesInfo(e, t) {
    var i, s, a;
    const r = null === (a = null === (s = null === (i = this.service) || undefined === i ? undefined : i.getBuildOption()) || undefined === s ? undefined : s.nativeLib) || undefined === a ? undefined : a.librariesInfo;
    if (r && r.length > 0) for (const e of r) e.linkLibraries && this.linkLibrariesMap.set(e.name, e.linkLibraries.join(";"));
    if (t || !fs_extra_1.existsSync(e)) return;
    const n = hvigor_1.ParseJsonFile.parseJsonFile(e).nativeComponents;
    if (n && n.length > 0) for (const e of n) e.name && e.linkLibraries && this.linkLibrariesMap.set(e.name, e.linkLibraries.join(";"));
  }
  getHeaderPath(e, t) {
    var i;
    const s = null === (i = null == t ? undefined : t.getBuildOption().nativeLib) || undefined === i ? undefined : i.headerPath;
    if (!s) return [path_1.resolve(e.getDependencyRootPath(), common_const_js_1.NativeConst.INCLUDE_DIR)];
    if ("string" == typeof s) return path_1.isAbsolute(s) ? [s] : [path_1.resolve(e.getDependencyRootPath(), s)];
    return s.map(t => path_1.isAbsolute(t) ? t : path_1.resolve(e.getDependencyRootPath(), t));
  }
}
export class ArchResolver {
  constructor(e, t) {
    this.libraries = [], this.cmakeText = "", this.arch = e, this.pkg = t, this.path = path_1.resolve(this.pkg.libsPath, e), this.cmakeFile = path_1.resolve(t.cmakeDir, e, "lib", cpu_abi_enum_js_1.CpuAbiEnum.getCpuType(e), "cmake", t.name, `${t.name}Config.cmake`);
  }
  static getInstance(e, t) {
    return new ArchResolver(e, t);
  }
  async build() {
    return this.libraries = await this.calLibraries(), this;
  }
  async calLibraries() {
    if (!this.pkg.service) {
      if (fs_extra_1.existsSync(this.path)) {
        log.debug(`Reading libs dir ${this.path}`);
        return glob_1.glob.sync(common_const_js_1.NativeConst.LIBRARY_FILE_PATTERN, {
          cwd: this.path
        }).filter(e => path_1.basename(e) !== common_const_js_1.NativeConst.SHARED_STL_LIBRARY).map(e => LibraryResolver.getInstance(path_1.basename(e), path_1.resolve(this.path, e), this));
      }
      return log.debug("No available libraries."), [];
    }
    const e = this.pkg.service.getTargetData(),
      t = e.getPathInfo().getNinjaWorkDir(),
      i = path_1.resolve(t, this.arch);
    if (log.debug(`Local module ${this.pkg.name}, reading cmake code model in ${i}.`), !fs_extra_1.existsSync(i)) return log.warn(`not found cmake code model in ${i}`), [];
    const s = [...cmake_util_js_1.CmakeUtil.parseLibraries(i, e.getTargetName(), this.arch).values()].filter(e => "STATIC_LIBRARY" === e.getType() || "SHARED_LIBRARY" === e.getType()),
      a = s.flatMap(e => {
        var t;
        return null !== (t = e.getOutputs()) && undefined !== t ? t : [];
      });
    return this.pkg.service.getModuleService().getModuleModel().isHspModule() && a.push(...s.flatMap(e => {
      var t;
      return null !== (t = e.getRuntimeFiles()) && undefined !== t ? t : [];
    })), a.map(e => path_1.isAbsolute(e) ? e : path_1.resolve(t, this.arch, e)).filter(e => e.includes(".so") || e.includes(".a")).map(e => LibraryResolver.getInstance(path_1.basename(e), path_1.resolve(e), this));
  }
}
export class LibraryResolver {
  constructor(e, t, i) {
    this.arch = i, this.nameOnDisk = e, this.name = this.tryGetLibraryName(), this.path = t, this.text = this.genPkgText();
  }
  static getInstance(e, t, i) {
    return new LibraryResolver(e, t, i);
  }
  genPkgText() {
    const e = this.arch.pkg.matchRule;
    if (e && this.isSoExcluded(e)) return os_1.EOL;
    const t = `${this.arch.pkg.name}::${this.name}`;
    let i = `if(NOT TARGET ${t})`;
    const s = this.nameOnDisk.includes(".a") ? "STATIC" : "SHARED";
    i += `${os_1.EOL}    add_library(${t} ${s} IMPORTED)`, i += `${os_1.EOL}    set_target_properties(${t} PROPERTIES`;
    const a = this.arch.pkg.headerPath.filter(e => fs_extra_1.existsSync(e) && fs_extra_1.readdirSync(e).length > 0).map(e => e.replace(/\\/g, "/"));
    return a.length > 0 && (i += `${os_1.EOL}        INTERFACE_INCLUDE_DIRECTORIES "${a.join(";")}"`), i += `${os_1.EOL}        IMPORTED_LOCATION "${this.path.replace(/\\/g, "/")}"`, this.arch.pkg.linkLibrariesMap.get(this.nameOnDisk) && (i += `${os_1.EOL}        INTERFACE_LINK_LIBRARIES "${this.arch.pkg.linkLibrariesMap.get(this.nameOnDisk)}"`), i += `)${os_1.EOL}endif()${os_1.EOL}`, i;
  }
  isSoExcluded(e) {
    var t, i;
    return (null === (t = e.excludePattern) || undefined === t ? undefined : t.some(e => (0, minimatch_1.minimatch)(this.path, e, {
      dot: true
    }))) || (null === (i = e.exclude) || undefined === i ? undefined : i.includes(this.nameOnDisk));
  }
  tryGetLibraryName() {
    const e = this.nameOnDisk.lastIndexOf(".so") > -1 ? this.nameOnDisk.lastIndexOf(".so") : this.nameOnDisk.lastIndexOf(".a"),
      t = this.nameOnDisk.toLowerCase().startsWith("lib") ? 3 : 0;
    return e > -1 ? this.nameOnDisk.substring(t, e) : this.nameOnDisk.substring(t);
  }
}
