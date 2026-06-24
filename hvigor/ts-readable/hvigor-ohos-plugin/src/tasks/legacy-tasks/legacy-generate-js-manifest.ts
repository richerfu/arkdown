/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-js-manifest.js
 */
import * as path from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as fse from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as inject_const_js_1 from "../../const/inject-const";
import * as ability_type_enum_js_1 from "../../enum/ability-type-enum";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as legacy_pre_build_js_1 from "./legacy-pre-build";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
const SRC_PATH = inject_const_js_1.InjectConst.PREVIEWER_REPLACE_SRC_PATH,
  UNIT_TEST_SRC_PATH = inject_const_js_1.InjectConst.UNIT_TEST_REPLACE_SRC_PATH,
  PAGE = inject_const_js_1.InjectConst.PREVIEWER_REPLACE_PAGE,
  UNIT_TEST_PAGE = inject_const_js_1.InjectConst.UNIT_TEST_REPLACE_PAGE,
  BUILD_ROOT = inject_const_js_1.InjectConst.BUILD_ROOT;
export class LegacyGenerateJsManifest extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, LegacyFATask.GENERATE_JS_MANIFEST), this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyGenerateJsManifest.name), this.previewSrcPath = hvigor_1.hvigorCore.getExtraConfig().get(SRC_PATH), this.unitTestSrcPath = hvigor_1.hvigorCore.getExtraConfig().get(UNIT_TEST_SRC_PATH), this.previewPage = hvigor_1.hvigorCore.getExtraConfig().get(PAGE), this.unitTestPage = hvigor_1.hvigorCore.getExtraConfig().get(UNIT_TEST_PAGE), this.isUnitTest = inject_util_js_1.InjectUtil.isUnitTestProcess(), this.isPreview = hvigor_1.hvigorCore.getExtraConfig().get(BUILD_ROOT) === build_directory_const_js_1.BuildDirConst.PREVIEW_BUILD_PATH, this._moduleModel = this.service.getModuleModel(), this._abilityModels = this._moduleModel.getLegacyAbilities(this.targetData.getTargetName()), this._configOpt = (0, wdk_1.cloneDeep)(this._moduleModel.getJsonObjByTargetName(this.targetData.getTargetName())), this._minPlatformVersion = this.targetData.getCompatibleApiVersion();
  }
  initTaskDepends() {
    this.declareDepends(legacy_pre_build_js_1.LegacyPreBuild.name);
  }
  declareInputs() {
    var e;
    const t = new Map(),
      i = null === (e = this.targetData.getTargetOpt().source) || undefined === e ? undefined : e.abilities,
      s = JSON.stringify(i || []),
      o = (0, hvigor_common_1.hash)(JSON.stringify(this._abilityModels)),
      n = (0, hvigor_common_1.hash)(JSON.stringify(this._configOpt)),
      r = (0, hvigor_common_1.hash)(s);
    return t.set("ABI_OBJ_HASH", o), t.set("CONFIG_OBJ", n), t.set("ABILITIES_HASH", r), t.set("HAS_LITE_DEVICE", this.service.hasLiteDevice()), t.set("MIN_PLATFORM_VERSION", this._minPlatformVersion), this.isPreview && (t.set("PREVIEWER_REPLACE_PAGE", JSON.stringify(hvigor_1.hvigorCore.getExtraConfig().get(PAGE))), t.set("PREVIEWER_REPLACE_SRCPATH", JSON.stringify(hvigor_1.hvigorCore.getExtraConfig().get(SRC_PATH)))), t;
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntry(this.targetData.getPathInfo().getIntermediatesLegacyManifestJson());
  }
  doTaskAction() {
    for (const e of this._abilityModels) e.getType() !== ability_type_enum_js_1.AbilityTypeEnum.PAGE && e.getType() !== ability_type_enum_js_1.AbilityTypeEnum.FORM || this.generateJsonInfo(e, this._configOpt, this.targetData);
  }
  getUnitTestManifestPage(e, t) {
    var i;
    if (!this.unitTestPage) {
      let s = this.getAbilityPages(e, t);
      return undefined === s && (s = null === (i = t.getConfigJsonJsObj()) || undefined === i ? undefined : i.pages), s || [];
    }
    return this._log.debug(`replace ${this.unitTestPage} for ${this.unitTestSrcPath} manifest.`), [this.unitTestPage];
  }
  getManifestPages(e, t) {
    var i;
    if (!this.isPreview || !this.previewPage || this.previewSrcPath && this.previewSrcPath !== t.getRelateSrcPath()) {
      let s = this.getAbilityPages(e, t);
      return undefined === s && (s = null === (i = t.getConfigJsonJsObj()) || undefined === i ? undefined : i.pages), s || [];
    }
    return this._log.debug(`replace ${this.previewPage} for ${this.previewSrcPath} manifest.`), [this.previewPage];
  }
  getAbilityPages(e, t) {
    const i = e.findTargetConfigOpt(t);
    if (i) return i.pages;
  }
  generateJsonInfo(e, t, i) {
    var s, o;
    let n = {
      appID: t.app.bundleName,
      versionName: t.app.version.name,
      versionCode: t.app.version.code,
      minPlatformVersion: this._minPlatformVersion,
      appName: LegacyGenerateJsManifest.findAppName(t),
      deviceType: t.module.deviceType,
      window: this.getWindowObj(e),
      pages: this.isUnitTest ? this.getUnitTestManifestPage(i, e) : this.getManifestPages(i, e)
    };
    const r = null === (s = e.getConfigJsonJsObj()) || undefined === s ? undefined : s.mode,
      a = e.getType() === ability_type_enum_js_1.AbilityTypeEnum.FORM ? ability_type_enum_js_1.AbilityTypeEnum.FORM : null === (o = e.getConfigJsonJsObj()) || undefined === o ? undefined : o.type;
    r ? n = {
      ...n,
      mode: r
    } : a && (n = {
      ...n,
      type: a
    });
    const _ = path.resolve(i.getPathInfo().getIntermediatesLegacyManifestJson(), e.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LEGACY_MANIFEST_JSON);
    fse.outputJSONSync(_, n);
  }
  static findAppName(e) {
    var t, i, s;
    const o = null !== (t = e.module.abilities) && undefined !== t ? t : [];
    for (const e of o) if (e.skills && (null === (i = e.skills[0]) || undefined === i ? undefined : i.actions) && "action.system.home" === (null === (s = e.skills[0]) || undefined === s ? undefined : s.actions[0])) return e.name;
    return o[0].label ? o[0].label : o[0].name;
  }
  getWindowObj(e) {
    var t, i;
    if (this.targetData.isSingleDeviceTypeTarget()) return this.service.hasLiteDevice() || null === (t = e.getConfigJsonJsObj()) || undefined === t ? undefined : t.window;
    {
      const t = null === (i = e.getConfigJsonJsObj()) || undefined === i ? undefined : i.window;
      return null != t ? t : {
        autoDesignWidth: false,
        designWidth: common_const_js_1.CommonConst.DEFAULT_DESIGN_WIDTH
      };
    }
  }
}
