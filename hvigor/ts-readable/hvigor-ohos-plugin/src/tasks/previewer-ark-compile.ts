/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/previewer-ark-compile.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as json_util_js_1 from "../utils/json-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ohmurl_utils_js_1 from "../utils/ohmurl-utils";
import * as reuse_watch_worker_js_1 from "../utils/previewer/reuse-watch-worker";
import * as task_util_js_1 from "../utils/task-util";
import * as ark_compile_js_1 from "./ark-compile";
import * as task_names_js_1 from "./common/task-names";
import * as preview_update_assets_js_1 from "./preview-update-assets";
var WorkerType,
  CommonTask = task_names_js_1.TaskNames.CommonTask;
!function (e) {
  e[e.INVALID = 0] = "INVALID", e[e.NORMAL = 1] = "NORMAL", e[e.WATCH = 2] = "WATCH";
}(WorkerType || (WorkerType = {}));
export class PreviewerArkCompile extends ark_compile_js_1.ArkCompile {
  constructor() {
    super(...arguments), this.logger = ohos_logger_js_1.OhosLogger.getLogger(PreviewerArkCompile.name);
  }
  declareInputFiles() {
    return this.moduleModel.isHarModule() ? super.declareInputFiles().deleteEntry(this.aceBuildJsonDir) : super.declareInputFiles();
  }
  async doTaskAction() {
    const e = hvigor_1.hvigorCore.getExtraConfig().get("pageType"),
      t = await this.getPreviewCompileConfig(this.logger),
      r = this.getWorkerType(e);
    if (r === WorkerType.NORMAL || r === WorkerType.WATCH) {
      const o = {
          modulePath: t.modulePath,
          pageType: null != e ? e : common_const_js_1.PageType.PAGE,
          pagesPath: this.getShouldCompilePages(null != e ? e : common_const_js_1.PageType.PAGE)
        },
        s = r === WorkerType.NORMAL;
      if (!(await reuse_watch_worker_js_1.ReuseWatchWorkerManager.reuseWatchWorker(o, s, this.getPreviewPagePath()))) {
        this.logger.debug("Can not reuse worker, create new worker for preview");
        const r = await this.generateWorkerConfig(t, e);
        reuse_watch_worker_js_1.ReuseWatchWorkerManager.createWorker(o, s, () => this.createPreviewWorker(s, r));
      }
    }
    this.addOhmurlToHarAbility(), (0, task_util_js_1.warnCreateBuildProfileTask)(this.targetData, this.targetService, this._log);
  }
  getPreviewPagePath() {
    const e = this.pathInfo.getBuildConfigPath(true);
    if (fs_extra_1.existsSync(e)) {
      return fs_extra_1.readJsonSync(e).previewPagePath || "";
    }
    return "";
  }
  getShouldCompilePages(e) {
    const t = this.pathInfo.getModulePreviewIntermediatesProfilePath(),
      r = this.moduleModel.getSourceRootByTargetName(this.targetName);
    if (e === common_const_js_1.PageType.PAGE) {
      const e = path_1.resolve(t, build_directory_const_js_1.BuildArtifactConst.MAIN_PAGES_JSON);
      if (fs_extra_1.existsSync(e)) {
        return fs_extra_1.readJsonSync(e).src.map(e => path_1.normalize(path_1.join(r, common_const_js_1.CommonConst.ETS, `${e}.ets`)));
      }
    } else {
      const e = path_1.resolve(t, build_directory_const_js_1.BuildArtifactConst.FORM_CONFIG_JSON);
      if (fs_extra_1.existsSync(e)) {
        return fs_extra_1.readJsonSync(e).forms.map(e => path_1.normalize(path_1.join(r, e.src)));
      }
    }
    return [];
  }
  async generateWorkerConfig(e, t) {
    var r;
    const o = this.getBlockDependencies().map(e => e.getDependencyName());
    return e.pkgNameToPkgBriefInfo = this.pkgNameToPkgBriefInfo, e.otherPaths = this.getTscAddressPaths(e.aceModuleRoot), e.compileBlockPkg = o, t && t !== common_const_js_1.PageType.PAGE ? {
      ...e,
      widgetCompile: "true",
      cachePath: path_1.resolve(null !== (r = e.cachePath) && undefined !== r ? r : this.getTaskTempDir(this.targetData, "_widget"), "widget"),
      port: undefined
    } : (e.transformAbc = !!this.getTransformAbcAction(), e);
  }
  getWorkerType(e) {
    if (e && e !== common_const_js_1.PageType.PAGE) return this.needSubmitArkTsWidget && e === common_const_js_1.PageType.CARD ? WorkerType.WATCH : WorkerType.INVALID;
    {
      const e = "true" === hvigor_1.hvigorCore.getExtraConfig().get("compileHspDependencies"),
        t = hvigor_1.hvigorCore.getExtraConfig().get("mainModule");
      return e && t !== this.moduleName ? WorkerType.NORMAL : WorkerType.WATCH;
    }
  }
  createPreviewWorker(e, t) {
    const r = e ? hvigor_1.normalWorker : hvigor_1.watchWorker,
      o = path_1.resolve(__dirname, "./worker/preview-build.js"),
      s = r.createWorker(this, o, true, e => this.handleCompileEvents(e, this.getSubDurationEvent(s), true), {
        workerData: {
          projectConfig: t,
          logLevel: this.logger.getLevel().levelStr
        }
      }, {
        customType: hvigor_arkts_compose_1.TRANSFORM_ABC_WORKER_REQUEST,
        action: this.doBeforeObfuscationAction.bind(this)
      });
    return s;
  }
  addOhmurlToHarAbility() {
    var e;
    const t = json_util_js_1.JsonUtil.getJson5Obj(this.pathInfo.getPreviewIntermediatesResModuleJsonPath()),
      r = this.moduleModel.getModule().getNodeDir();
    for (const o of [common_const_js_1.AbilityConst.UI_ABILITY, common_const_js_1.AbilityConst.EXTENSION_ABILITY]) {
      const s = null === (e = null == t ? undefined : t.module) || undefined === e ? undefined : e[o];
      if (!(null == s ? undefined : s.length)) continue;
      const i = this.getDependencyHarAbility(o);
      null == s || s.forEach(e => {
        if (!e.srcEntry) return;
        if (o === common_const_js_1.AbilityConst.EXTENSION_ABILITY && e.srcEntry.endsWith(".so")) return;
        const t = path_1.resolve(r, build_directory_const_js_1.BuildDirConst.SRC_MAIN, e.srcEntry);
        Object.assign(e, {
          ohmurl: (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(this.targetService.getPkgContextInfo(this.packageJsonObj.name), t.substring(r.length + 1))
        });
        const s = i.find(t => t.name === e.name);
        s && Object.assign(e, s);
      }), Object.assign(t.module, {
        [o]: s
      });
    }
    fs_extra_1.writeFileSync(this.pathInfo.getPreviewIntermediatesResModuleJsonPath(), JSON.stringify(t));
  }
  getDependencyHarAbility(e) {
    const t = path_1.resolve(this.moduleModel.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN),
      r = [];
    return this.service.getHarDependencies().forEach(o => {
      const s = o.getModuleJsonObj(),
        i = null == s ? undefined : s.module[e];
      (null == i ? undefined : i.length) && i.forEach(e => {
        const s = (0, wdk_1.cloneDeep)(e);
        if (!s.srcEntry) return;
        const i = path_1.resolve(o.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, s.srcEntry);
        Object.assign(s, {
          srcEntry: path_1.relative(t, i),
          ohmurl: (0, ohmurl_utils_js_1.generateOhmurlForSrcEntry)(this.targetService, this.moduleName, o, s.srcEntry)
        }), r.push(s);
      });
    }), r;
  }
  allowToPreloadSystemSo() {
    return false;
  }
  initTaskDepends() {
    this.declareDepends(preview_update_assets_js_1.PreviewUpdateAssets.name), (0, task_util_js_1.limitMinApiVersion)(this.targetData, sdk_const_js_1.ApiVersion.API_VERSION_9) && this.declareDepends(CommonTask.CREATE_BUILD_PROFILE.name);
  }
}
