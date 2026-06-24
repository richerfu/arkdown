/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/file-util.js
 */
import fs_1 from "fs";
import promises_1 from "fs/promises";
import * as node_fs_1 from "node:fs";
import os_1 from "os";
import path_1 from "path";
import * as promises_2 from "stream/promises";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as task_util_1 from "@ohos/hvigor/src/base/util/task-util";
import fs_extra_1 from "fs-extra";
import * as glob_1 from "glob";
import * as common_const_js_1 from "../const/common-const";
import * as dependency_param_validate_1 from "../validate/dependency-param-validate";
import * as json_util_js_1 from "./json-util";
import * as oh_package_loader_js_1 from "./loader/file/oh-package-loader";
import * as ohos_logger_js_1 from "./log/ohos-logger";
export class FileUtil {
  static async linkFile(t, e, a = false) {
    (await FileUtil.exists(t)) || this._log.printErrorExit("CANNOT_FIND_SO_SOURCE", [t], [[t]]);
    const s = await promises_1.lstat(t);
    if (s.isDirectory()) return void this._log.debug(`Skip copying ${t}, as it is a folder.`);
    if (!fs_1.existsSync(t)) return void this._log.debug(`Non-existent file or symbolic link ${t}.`);
    const i = a && s.isSymbolicLink() ? path_1.resolve(path_1.dirname(t), await promises_1.readlink(t)) : t;
    await fs_extra_1.ensureDir(path_1.dirname(e));
    try {
      if (await promises_1.access(t, fs_1.constants.R_OK), (await FileUtil.exists(e)) && (await promises_1.unlink(e)), await promises_1.link(t, e), this._log.debug(`Link ${i} to ${e} successfully.`), a && s.isSymbolicLink()) {
        const s = path_1.resolve(path_1.dirname(e), await promises_1.readlink(t));
        await this.linkFile(i, s, a);
      }
    } catch (a) {
      this._log.debug(`Hard link from ${t} to ${e} failed. Do copy instead.`), await promises_1.copyFile(i, e);
    }
  }
  static async exists(t) {
    try {
      return await promises_1.lstat(t), true;
    } catch (t) {
      return false;
    }
  }
  static isDirectory(...t) {
    return fs_extra_1.statSync(path_1.resolve(...t)).isDirectory();
  }
  static checkDirWithoutDelete(...t) {
    const e = path_1.resolve(...t);
    fs_extra_1.existsSync(e) || fs_extra_1.mkdirSync(e, {
      recursive: true
    });
  }
  static checkFile(...t) {
    const e = path_1.resolve(...t);
    fs_extra_1.existsSync(e) || FileUtil.makeFile(e), fs_extra_1.removeSync(e), FileUtil.makeFile(e);
  }
  static makeFile(t, e = "") {
    fs_extra_1.writeFileSync(t, e);
  }
  static deleteFile(t) {
    try {
      fs_extra_1.existsSync(t) && fs_extra_1.removeSync(t);
    } catch (e) {
      this._log.printErrorExit("DELETE_FILE_ERROR", [t]);
    }
  }
  static readFile(t) {
    try {
      if (!fs_extra_1.existsSync(t)) return;
      return fs_extra_1.readFileSync(t, {
        encoding: "utf-8"
      });
    } catch (e) {
      return void this._log.error(`Please check file at ${t}.`);
    }
  }
  static readLines(t) {
    try {
      if (!fs_extra_1.existsSync(t)) return;
      return fs_extra_1.readFileSync(t, {
        encoding: "utf-8"
      }).split(os_1.EOL).filter(t => "" !== t);
    } catch (e) {
      return void this._log.error(`readLines failed Please check file at ${t}.`);
    }
  }
  static checkPathLength(t) {
    const e = (0, hvigor_1.maxPathLength)();
    t.length > e && this._log.printErrorExit("FILE_PATH_EXCEED_LIMITATION", [`${e}`]);
  }
  static async hashEntry(t, e = "**/*") {
    const a = new Map(),
      s = glob_1.glob.sync(e, {
        nodir: true,
        absolute: true,
        cwd: t,
        follow: false
      });
    for (const t of s) {
      const e = await this.pipelineAsync(t);
      a.set(t, e);
    }
    return a;
  }
  static async pipelineAsync(t) {
    var e;
    return (null === (e = null === hvigor_1.hvigorCore || undefined === hvigor_1.hvigorCore ? undefined : hvigor_1.hvigorCore.getParameter()) || undefined === e ? undefined : e.getPropertyInternal(common_const_js_1.CommonConst.OHOS_PROCESSLIB_OPTIMIZATION)) ? (this._log.debug(`Check size & timestamp for ${t}`), this.hashStats(t)) : (this._log.debug(`Check content hash for ${t}`), await this.hashContent(t));
  }
  static async hashContent(t) {
    if (!fs_extra_1.existsSync(t)) throw new Error(`File "${t}" does not exist`);
    const e = (0, hvigor_common_1.createHash)();
    return await (0, promises_2.pipeline)((0, node_fs_1.createReadStream)(t), e), e.digest("hex");
  }
  static hashStats(t) {
    if (!fs_extra_1.existsSync(t)) throw new Error(`File "${t}" does not exist`);
    const e = (0, hvigor_common_1.createHash)(),
      a = fs_1.statSync(t);
    return e.update(a.size.toString() + a.mtimeMs), e.digest("hex");
  }
  static convertToAbsolutePath(t, e) {
    return path_1.isAbsolute(t) ? t : path_1.resolve(e, t);
  }
  static convertToAbsolutePaths(t, e) {
    return t.map(t => FileUtil.convertToAbsolutePath(t, e));
  }
  static isSubDir(t, e) {
    const a = path_1.relative(t, e);
    return a && !a.startsWith("..") && !path_1.isAbsolute(a);
  }
  static fileExists(t) {
    return fs_1.existsSync(t) && fs_1.realpathSync.native(t) === t;
  }
  static matchingFile(t, e = false) {
    return !!glob_1.glob.sync(t, {
      nocase: e
    }).length;
  }
  static copySpecialFileToTempDir(t, e) {
    fs_extra_1.readdirSync(t).forEach(a => {
      if (!this.checkSpecialFile(a)) return;
      const s = path_1.resolve(t, a);
      try {
        fs_extra_1.statSync(s).isFile() && fs_extra_1.copySync(s, path_1.resolve(e, a));
      } catch (t) {
        this._log.printErrorExit("COPY_FILE_ERROR", [s]);
      }
    });
  }
  static disposalPackingOptionAsset(t, e, a) {
    var s, i;
    ((null === (s = null == e ? undefined : e.include) || undefined === s ? undefined : s.length) || (null === (i = null == e ? undefined : e.exclude) || undefined === i ? undefined : i.length)) && (this.disposalPackingOptInclude(e.include, t, a), this.disposalPackingOptExclude(e.exclude, a));
  }
  static disposalPackingOptInclude(t, e, a) {
    if (!(null == t ? undefined : t.length)) return;
    const s = ["node_modules/**", "oh_modules/**", ".preview/**", "build/**", ".cxx/**", ".test/**"],
      i = path_1.resolve(e),
      r = new Set();
    t.forEach(t => {
      const a = glob_1.glob.sync(t, {
        cwd: e,
        ignore: s
      });
      (null == a ? undefined : a.length) && a.forEach(t => {
        r.add(t);
      });
    }), r.forEach(t => {
      const s = path_1.resolve(e, t);
      fs_extra_1.existsSync(s) && s.startsWith(i) && fs_extra_1.copySync(s, path_1.resolve(a, t));
    });
  }
  static disposalPackingOptExclude(t, e) {
    if (!(null == t ? undefined : t.length)) return;
    const a = ["oh-package.json5", "ResourceTable.txt", "Index.d.ets", "ets/**", "src/main/module.json"],
      s = path_1.resolve(e);
    t.forEach(t => {
      const i = glob_1.glob.sync(t, {
        cwd: e,
        ignore: a
      });
      (null == i ? undefined : i.length) && i.forEach(t => {
        const a = path_1.resolve(e, t);
        fs_extra_1.existsSync(a) && a.startsWith(s) && fs_extra_1.removeSync(a);
      });
    });
  }
  static async readDirFileNames(t) {
    const e = new Set();
    return await async function t(a) {
      try {
        const s = await fs_extra_1.readdir(a, {
          withFileTypes: true
        });
        for (const i of s) {
          const s = path_1.join(a, i.name);
          i.isDirectory() ? await t(s) : e.add(i.name);
        }
      } catch (t) {}
    }(t), e;
  }
  static addParamToOhPack(t, e, a, s, i, r) {
    const o = t ? common_const_js_1.CommonConst.OH_PACKAGE_JSON5 : common_const_js_1.CommonConst.PACKAGE_JSON,
      n = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(e, o));
    let l = json_util_js_1.JsonUtil.getJson5Obj(n);
    l.metadata ? Object.assign(l.metadata, a) : Object.assign(l, {
      metadata: a
    }), l = FileUtil.mergeExtraParamsToOhPack(l, s, i, r), fs_extra_1.writeJSONSync(n, l);
  }
  static mergeExtraParamsToOhPack(t, e, a, s) {
    return t = {
      ...t,
      ...e,
      nativeComponents: (t.nativeComponents || []).concat(e.nativeComponents || [])
    }, e.nativeComponents || delete t.nativeComponents, t.nativeComponents && (t.nativeComponents = t.nativeComponents.filter(t => s.has(t.name))), a === (0, task_util_1.getAlignTarget)() && (t.version += `+${(0, task_util_1.getAlignTarget)()}`), t;
  }
  static checkSpecialFile(t) {
    const e = t.toLowerCase(),
      a = e.split(".")[0];
    return -1 !== ["readme.opensource"].indexOf(e) || -1 !== ["license"].indexOf(a) || /^(changelog|readme)([-_](cn|en))?\.md$/i.test(e);
  }
  static uniqueFileName(t, e) {
    const a = new Map();
    if (t.forEach(t => {
      a.set(t, 1);
    }), a.has(e)) {
      let t = a.get(e);
      for (; a.has(this.addSuffix(e, t));) t++;
      a.set(e, t + 1);
      const s = this.addSuffix(e, t);
      return a.set(s, 1), s;
    }
    return a.set(e, 1), e;
  }
  static addSuffix(t, e) {
    return `${t}${e}`;
  }
  static traverseFileFolder(t) {
    const e = [];
    if (!fs_extra_1.existsSync(t) || !fs_extra_1.statSync(t).isDirectory()) return [];
    return fs_extra_1.readdirSync(t).forEach(a => {
      const s = path_1.join(t, a),
        i = fs_extra_1.statSync(s);
      i.isDirectory() ? this.traverseFileFolder(s) : i.isFile() && e.push(a.split(".")[0]);
    }), e;
  }
  static getAllFilesFromFolder(t) {
    const e = [];
    if (!fs_extra_1.existsSync(t) || !fs_extra_1.statSync(t).isDirectory()) return [];
    return fs_extra_1.readdirSync(t).forEach(a => {
      const s = path_1.join(t, a),
        i = fs_extra_1.statSync(s);
      i.isDirectory() ? e.push(...this.getAllFilesFromFolder(s)) : i.isFile() && e.push(s);
    }), e;
  }
  static getFileSuffix(t) {
    const e = path_1.parse(t).base;
    return "" === e || -1 === e.indexOf(".") ? e : e.substring(e.lastIndexOf(".") + 1);
  }
  static normalizePathSeparator(t) {
    return t.replace(/\\\\/g, "/").replace(/\\/g, "/").replace(/\/:/g, ":");
  }
  static resolveJsonObjWithoutParam(t, e, a, s, i) {
    const r = [];
    for (const o in t) if (!s || s.includes(o)) if ("object" == typeof t[o] && null !== t[o]) {
      const s = this.resolveJsonObjWithoutParam(t[o], e, a, undefined, i);
      r.push(...s);
    } else if ("string" == typeof t[o]) {
      const s = FileUtil.extractValue(t[o], e, a, i, o);
      t[o] = s.value, s.localDepArr && r.push(...s.localDepArr);
    }
    return r;
  }
  static extractValue(t, e, a, s, i) {
    const r = t.match(common_const_js_1.ParameterizationConst.REGEX);
    if (!r) return {
      value: t
    };
    e || this._log.printErrorExit("PARAMETER_FILE_KEY_ERROR", [t, a.parameterFilePath]);
    const o = r[1].split(".");
    return (0, dependency_param_validate_1.matchValueWithParameterizationKey)({
      keys: o,
      parameterFileJson: e,
      pathParamsObj: a,
      dependencyName: i,
      ohPackageInfoGetter: s
    });
  }
  static extracted(t, e, a, s) {
    return FileUtil.extractValue(t, e, a, undefined, s).value;
  }
  static copyDir(t, e) {
    const a = fs_extra_1.readdirSync(t, {
      withFileTypes: true
    });
    fs_extra_1.mkdirsSync(e);
    for (const s of a) {
      const a = path_1.join(t, s.name),
        i = path_1.join(e, s.name);
      s.isDirectory() ? this.copyDir(a, i) : fs_extra_1.copyFileSync(a, i);
    }
  }
  static async isEmptyDir(t) {
    if (!fs_extra_1.existsSync(t)) return true;
    try {
      return 0 === (await fs_extra_1.readdir(t)).length;
    } catch (e) {
      return this._log.debug(`failed to readdir ${t}`), false;
    }
  }
  static getRemoveExtPath(t) {
    const e = path_1.dirname(t),
      a = path_1.basename(t, path_1.extname(t));
    return path_1.join(e, a);
  }
}
FileUtil._log = ohos_logger_js_1.OhosLogger.getLogger("File");
