/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-ark-compile.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as ability_type_enum_js_1 from "../../enum/ability-type-enum";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as run_ark_js_1 from "../worker/run-ark";
import * as legacy_compile_node_js_1 from "./legacy-compile-node";
import * as worker_id_helper_1 from "../worker/worker-id-helper";
export class LegacyArkCompile extends legacy_compile_node_js_1.LegacyCompileNode {
  constructor(e, t, o = task_names_js_1.TaskNames.LegacyFATask.COMPILE_ARK) {
    super(e, t, o);
  }
  async doTaskAction() {
    const e = ohos_logger_js_1.OhosLogger.getLogger(LegacyArkCompile.name),
      t = this.moduleModel.getLegacyAbilities(this.targetName),
      o = this.sdkInfo.isOhos ? common_const_js_1.CommonConst.OPEN_HARMONY : common_const_js_1.CommonConst.HARMONY_OS,
      i = `${this.sdkInfo.isOhos}:${this.sdkInfo.getSdkVersion()}:${this.sdkInfo.getEtsComponentVersion()}:${this.sdkInfo.getEtsComponentReleaseType()}`;
    this.validateAbility(this.log);
    for (let s = 0; s < t.length; s++) {
      const r = t[s];
      if (r.getSrcLanguage() !== this.codeType) continue;
      const a = this.initDefaultLegacyArkConfig(o, i, r);
      this.log.debug("build config:"), this.log.debug(a);
      let l = null;
      l = await (0, run_ark_js_1.submitArkCompileWork)(this, this.getWorkerPool(), e, a, t => {
        this.moveReleaseMap(this.targetData, e, `js/${r.getRelateSrcPath()}`), this.handleCompileEvents(t, l ? this.getSubDurationEvent(l) : this.durationEvent, false);
      }, 0 === this.getWorkerPool().getMaxPoolNum() ? undefined : [(0, hvigor_common_1.mod)((0, worker_id_helper_1.getModuleIndex)(a.modulePath), this.getWorkerPool().getMaxPoolNum())]);
    }
    await this.arkCompileTestRunner(o, i, e);
  }
  async arkCompileTestRunner(e, t, o) {
    const i = this.needCompileTestRunner();
    if (!i[0]) return;
    const s = i[1],
      r = this.initTestRunnerLegacyArkConfig(e, t, s);
    let a = null;
    a = await (0, run_ark_js_1.submitArkCompileWork)(this, this.getWorkerPool(), o, r, e => this.handleCompileEvents(e, a ? this.getSubDurationEvent(a) : this.durationEvent, false), 0 === this.getWorkerPool().getMaxPoolNum() ? undefined : [(0, hvigor_common_1.mod)((0, worker_id_helper_1.getModuleIndex)(r.modulePath), this.getWorkerPool().getMaxPoolNum())]);
  }
  initCommonLegacyArkConfig(e, t) {
    var o;
    return {
      ...super.initCommonArkConfig(),
      appResource: this.resourceTablePath,
      rawFileResource: this.rawFileResource,
      resourceTableHash: (0, hvigor_1.hashFile)(this.resourceTablePath),
      runtimeOS: e,
      sdkInfo: t,
      compileMode: null === (o = this.moduleModel.getProfileOpt().buildOption) || undefined === o ? undefined : o.compileMode
    };
  }
  initTestRunnerLegacyArkConfig(e, t, o) {
    return {
      ...this.initCommonLegacyArkConfig(e, t),
      abilityType: ability_type_enum_js_1.AbilityTypeEnum.TEST_RUNNER,
      aceModuleRoot: path_1.resolve(this.sourcePath, o),
      aceModuleBuild: path_1.resolve(this.aceModuleBuild, o),
      cachePath: this.getLegacyArkCachePath(o),
      supportChunks: false
    };
  }
  initDefaultLegacyArkConfig(e, t, o) {
    return {
      ...this.initCommonLegacyArkConfig(e, t),
      abilityType: o.getType(),
      aceManifestPath: path_1.resolve(this.manifestDir, o.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LEGACY_MANIFEST_JSON),
      aceModuleRoot: path_1.resolve(this.sourcePath, o.getRelateSrcPath()),
      aceModuleBuild: path_1.resolve(this.aceModuleBuild, o.getRelateSrcPath()),
      cachePath: this.getLegacyArkCachePath(o.getRelateSrcPath()),
      aceSuperVisualPath: path_1.resolve(this.aceSuperVisualPath, o.getRelateSrcPath()),
      aceBuildJson: path_1.resolve(this.aceBuildJsonDir, o.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LOADER_JSON),
      supportChunks: o.getType() === ability_type_enum_js_1.AbilityTypeEnum.PAGE,
      sdkPath: this.sdkInfo.sdkDir
    };
  }
  getLegacyArkCachePath(e) {
    const t = path_1.resolve(this.getTaskTempDir(this.targetData), e);
    return fs_extra_1.ensureDirSync(t), t;
  }
}
