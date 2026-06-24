/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/prepare-quickfix.js
 */
import promises_1 from "fs/promises";
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as quick_fix_enum_js_1 from "../enum/quick-fix-enum";
import * as file_util_js_1 from "../utils/file-util";
import * as json_util_js_1 from "../utils/json-util";
import * as libs_file_cache_util_js_1 from "../utils/libs-file-cache-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as assemble_hqf_js_1 from "./hook/assemble/assemble-hqf";
import * as sign_util_js_1 from "./sign/sign-util";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
const logger = ohos_logger_js_1.OhosLogger.getLogger("PrepareQuickfix");
export class PrepareQuickfix extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.PREPARE_QUICKFIX), this.debugSymbol = this.targetService.getNativeLibOption().debugSymbol, this.libsDir = this.pathInfo.getIntermediatesStrippedLibsDir(), this.libsCachePath = path_1.resolve(this.pathInfo.getPatchDir(), "base_native_libs.json"), this.lastCachePath = path_1.resolve(this.pathInfo.getPatchDir(), "libs.json");
  }
  async doTaskAction() {
    this.initPatchConfig();
    const e = await this.collectNativeLibs(),
      [t, s] = await this.collectEts();
    await this.collectResources(), await this.logQuickfixChanges(e, t, s), await libs_file_cache_util_js_1.LibsFileCacheUtil.refreshLibsFileCache(this.pathInfo.getIntermediatesProcessLibs(), this.libsDir, this.lastCachePath, this.debugSymbol), await this.generatePatchJson();
  }
  initPatchConfig() {
    const e = this.pathInfo.getLoaderJsonPath();
    if (!fs_extra_1.existsSync(e)) return;
    const t = json_util_js_1.JsonUtil.getJson5Obj(e);
    t && t.patchConfig && (this.patchConfig = t.patchConfig);
  }
  async collectNativeLibs() {
    const e = path_1.resolve(this.pathInfo.getPatchDir(), "libs");
    await fs_extra_1.emptydir(e);
    const t = fs_extra_1.existsSync(this.libsCachePath) ? hvigor_1.Json5Reader.getJson5Obj(this.libsCachePath) : {
        stripped: {},
        libs: {}
      },
      s = fs_extra_1.existsSync(this.lastCachePath) ? hvigor_1.Json5Reader.getJson5Obj(this.lastCachePath) : {
        stripped: {},
        libs: {}
      },
      i = fs_extra_1.existsSync(this.libsDir) ? await libs_file_cache_util_js_1.LibsFileCacheUtil.generateFileHashesForDirectory(this.libsDir) : {},
      a = this.compareLibs(t.stripped, s.stripped, i);
    await fs_extra_1.emptydir(e);
    return await fs_extra_1.copy(this.libsDir, e, {
      filter: e => fs_extra_1.statSync(e).isDirectory() || Object.entries(a).some(([t, s]) => "" === path_1.relative(e, t) && s > quick_fix_enum_js_1.QuickFixEnum.UNCHANGED),
      recursive: true,
      overwrite: true
    }), a;
  }
  async collectResources() {
    var e, t, s;
    const i = path_1.resolve(this.pathInfo.getPatchDir(), "resources"),
      a = path_1.resolve(i, "resfile"),
      r = path_1.resolve(i, "rawfile");
    if (await fs_extra_1.emptydir(a), await fs_extra_1.emptydir(r), (null === (e = this.patchConfig) || undefined === e ? undefined : e.changedFileList) && fs_extra_1.existsSync(this.patchConfig.changedFileList)) {
      const e = json_util_js_1.JsonUtil.getJson5Obj(this.patchConfig.changedFileList);
      await this.copyResources(i, null === (t = e.resources) || undefined === t ? undefined : t.resFile), await this.copyResources(i, null === (s = e.resources) || undefined === s ? undefined : s.rawFile);
    }
  }
  async copyResources(e, t) {
    if (t && (null == t ? undefined : t.length)) for (const {
      filePath: s,
      resourcePath: i
    } of t) if (".DS_Store" !== path_1.basename(s) && fs_extra_1.existsSync(s)) {
      const t = path_1.join(e, path_1.relative(i, s));
      fs_extra_1.copySync(s, t);
    }
  }
  async collectEts() {
    var e, t;
    file_util_js_1.FileUtil.checkDirWithoutDelete(path_1.resolve(this.pathInfo.getPatchDir(), "ets"));
    const s = path_1.resolve(this.pathInfo.getPatchDir(), "ets.json");
    let i = false,
      a = false;
    if ((null === (e = this.patchConfig) || undefined === e ? undefined : e.changedFileList) && fs_extra_1.existsSync(this.patchConfig.changedFileList)) {
      const e = fs_extra_1.statSync(this.patchConfig.changedFileList);
      let r = {};
      if (fs_extra_1.existsSync(s) && (r = json_util_js_1.JsonUtil.getJson5Obj(s), a = null !== (t = r.isImportNewFile) && undefined !== t ? t : a), (null == r ? undefined : r.changedFileList) !== String(e.mtimeMs)) return logger.debug("changedFileList had been changed"), r.changedFileList = String(e.mtimeMs), fs_extra_1.writeJSONSync(s, r), i = true, [i, a];
    }
    return [i, a];
  }
  async logQuickfixChanges(e, t, s) {
    var i, a;
    let r = null !== (i = (0, wdk_1.max)(Object.values(e))) && undefined !== i ? i : quick_fix_enum_js_1.QuickFixEnum.UNCHANGED;
    if (r !== quick_fix_enum_js_1.QuickFixEnum.UNCHANGED && logger.debug(`native files had been changed, state is ${r}`), t && (r = quick_fix_enum_js_1.QuickFixEnum.LAST, logger.debug("arkTs files or resfile had been changed")), r === quick_fix_enum_js_1.QuickFixEnum.UNCHANGED && (null === (a = this.patchConfig) || undefined === a ? undefined : a.changedFileList) && fs_extra_1.existsSync(this.patchConfig.changedFileList)) {
      const e = json_util_js_1.JsonUtil.getJson5Obj(this.patchConfig.changedFileList);
      r = this.isChanged(e) ? quick_fix_enum_js_1.QuickFixEnum.BASE : r;
    }
    await fs_extra_1.ensureDir(this.pathInfo.getModuleBuildOutputPath()), await fs_extra_1.writeJSON(path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), "quickfix.json"), {
      changes: e,
      state: r,
      isImportNewFile: s,
      file: (0, sign_util_js_1.getHqfName)(this.moduleName, this.targetName, false)
    });
  }
  compareLibs(e, t, s) {
    const i = {};
    return Object.entries(s).forEach(([s, a]) => i[s] = t[s] && t[s] !== a ? quick_fix_enum_js_1.QuickFixEnum.LAST : e[s] && e[s] === a ? quick_fix_enum_js_1.QuickFixEnum.UNCHANGED : t[s] ? quick_fix_enum_js_1.QuickFixEnum.BASE : quick_fix_enum_js_1.QuickFixEnum.LAST), i;
  }
  async generatePatchJson() {
    const e = await PrepareQuickfix.getCurrentHqfVersion(),
      t = this.projectModel.getAppRes().getAppResOpt(),
      s = {
        app: {
          bundleName: t.app.bundleName,
          versionCode: t.app.versionCode,
          versionName: t.app.versionName,
          patchVersionCode: e,
          patchVersionName: String(e)
        },
        module: {
          name: this.moduleName,
          type: "patch",
          deviceTypes: this.moduleModel.getDeviceTypes(),
          originalModuleHash: ""
        }
      },
      i = this.pathInfo.getPatchDir();
    try {
      await promises_1.access(i);
    } catch (e) {
      await promises_1.mkdir(i);
    }
    await promises_1.writeFile(path_1.resolve(i, "patch.json"), JSON.stringify(s)), logger.debug((0, hvigor_1.replaceBundleName)(`Prepare ${t.app.bundleName} quickfix with patch versionCode ${e}.`, t.app.bundleName));
  }
  initTaskDepends() {
    this.declareDepends("ColdReloadArkTS"), this.declareDepends(task_names_js_1.TaskNames.Task.DO_NATIVE_STRIP.name);
  }
  getPatchConfigMode() {
    var e;
    const t = this.pathInfo.getBuildConfigPath();
    if (fs_extra_1.existsSync(t)) {
      const s = fs_extra_1.readJsonSync(t);
      return null === (e = null == s ? undefined : s.patchConfig) || undefined === e ? undefined : e.mode;
    }
    logger.error(`${t} is not exists`);
  }
  isChanged(e) {
    if (Array.isArray(e)) return e.length > 0;
    if ("object" == typeof e) for (const t of Array.from(Object.values(e))) if (this.isChanged(t)) return true;
    return false;
  }
  static async getCurrentHqfVersion() {
    const e = path_1.resolve(hvigor_1.PathUtil.getHvigorCacheDir(logger), "outputs", "patch", "patch.json");
    if (!fs_extra_1.existsSync(e)) return assemble_hqf_js_1.AssembleHqf.versionCode = 3e6, assemble_hqf_js_1.AssembleHqf.versionCode;
    if (assemble_hqf_js_1.AssembleHqf.versionCode) return assemble_hqf_js_1.AssembleHqf.versionCode;
    const t = await fs_extra_1.readJSON(e);
    return assemble_hqf_js_1.AssembleHqf.versionCode = parseInt((null == t ? undefined : t.patchVersionCode) ? t.patchVersionCode : 3e6), assemble_hqf_js_1.AssembleHqf.versionCode;
  }
}
