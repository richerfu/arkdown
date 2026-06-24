/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract-make-pack-info.js
 */
import * as wdk_1 from "@baize/wdk";
import * as fse from "fs-extra";
import path_1 from "path";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
import * as hvigor_1 from "@ohos/hvigor";
import * as device_type_const_js_1 from "../const/device-type-const";
import * as module_task_service_js_1 from "./service/module-task-service";
import * as abstract_merge_profile_js_1 from "./abstract-merge-profile";
export class AbstractMakePackInfo extends ohos_hap_task_js_1.OhosHapTask {
  declareInputs() {
    var e, t, o;
    const s = new Map(),
      a = (0, wdk_1.cloneDeep)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt),
      i = null !== (t = null == a ? undefined : a.bundleName) && undefined !== t ? t : this.targetData.getProduct().bundleName;
    i && s.set("bundleName", i);
    const r = null === (o = this.targetData.getTargetOpt().output) || undefined === o ? undefined : o.artifactName;
    r && s.set("artifactName", r);
    const n = this.targetData.getTargetDeviceType();
    s.set("deviceTypes", n), s.set("compileSdkVersion", this.compileApiVersion), s.set("compatibleSdkVersion", this.compatibleApiVersion);
    const l = this.moduleModel.getJsonProfileOptByTargetName(this.targetName, this.isFaMode);
    if (undefined !== l.module.packageName && delete l.module.packageName, undefined !== l.module.installationFree && delete l.module.installationFree, s.set(abstract_merge_profile_js_1.INPUT_KEY.moduleJsonOpt, JSON.stringify(l)), !this.isFaMode) return s;
    const d = this.service.getRelatedEntryModules(),
      u = this.service.getModuleModel().getModuleType();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(u, d)) for (const e of d) if (this.projectModel.getModuleModelByName(e)) {
      const t = this.targetData.findTargetDataByName(e),
        o = this.projectModel.getModuleModelByName(e);
      s.set(e, !!t && this.getEntryDeviceType(t, o));
    }
    return s;
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet(),
      t = this.service.getRelatedEntryModules(),
      o = this.service.getModuleModel().getModuleType();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(o, t)) {
      for (const o of t) if (this.projectModel.getModuleModelByName(o)) {
        const t = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), o, build_directory_const_js_1.BuildArtifactConst.PACK_INFO);
        e.addEntry(t);
      }
    } else {
      const t = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), build_directory_const_js_1.BuildArtifactConst.PACK_INFO);
      e.addEntry(t);
    }
    return e;
  }
  doMakePackInfo(e, t, o) {
    const s = this.targetData.getTargetDeviceType(),
      a = this.targetData.getTargetName(),
      i = this.moduleModel.getJsonObjByTargetName(a),
      r = this.getModuleObj(s, this.moduleModel, o, a),
      n = this.service.getRelatedEntryModules(),
      l = a === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET ? this.getModuleObj(s, this.moduleModel, o, common_const_js_1.DefaultTargetConst.DEFAULT_TARGET) : undefined,
      d = this.service.getModuleModel().getModuleType();
    if (!module_task_service_js_1.ModuleTaskService.checkEntryModules(d, n)) {
      const o = l ? [r, l] : [r];
      return void this.buildPackInfo(e, "", i, t, o, undefined);
    }
    for (const d of n) if (this.projectModel.getModuleModelByName(d)) {
      const n = this.targetData.findRelatedTargetData(d),
        u = this.projectModel.getModuleModelByName(d),
        c = this.getEntryDeviceType(n, u),
        _ = [r, this.getModuleObj(this.getDeviceTypeSubset(e, d, c, s), u, o, a)];
      if (l) {
        const t = this.getModuleObj(this.getDeviceTypeSubset(e, d, c, s), u, o, common_const_js_1.DefaultTargetConst.DEFAULT_TARGET);
        _.push(l, t);
      }
      this.buildPackInfo(e, d, i, t, _, u.getJsonObjByTargetName(a));
    }
  }
  buildPackInfo(e, t, o, s, a, i) {
    var r, n, l, d;
    const u = this.targetData.getModuleTargetOutputFileName(t, null),
      c = null !== (n = null === (r = this.targetData.getTargetOpt().output) || undefined === r ? undefined : r.artifactName) && undefined !== n ? n : u.substring(0, u.lastIndexOf(build_directory_const_js_1.BuildArtifactExtension.DOT_HAP)),
      _ = this.targetData.getModuleTargetOutputFileName(t, null, undefined, device_type_const_js_1.DeviceTypeConst.LITE),
      g = _.substring(0, _.lastIndexOf(build_directory_const_js_1.BuildArtifactExtension.DOT_HAP)),
      p = i ? this.getPackageObj(null !== (l = this.targetData.findTargetDataByName(t)) && undefined !== l ? l : this.targetData, o, c, g) : this.getPackageObj(null !== (d = this.targetData.findTargetDataByName(t)) && undefined !== d ? d : this.targetData, o, c, g),
      h = this.getPackInfoObj(s, a, p);
    this.outputJSON(e, h, this.targetData.getPathInfo().getModuleBuildOutputPath(), t);
  }
  processForms(e) {
    if (undefined === e) return;
    const t = [];
    for (let o = 0; o < e.length; o++) {
      const s = e[o],
        a = {
          name: s.name,
          type: "JS",
          updateEnabled: s.updateEnabled,
          scheduledUpdateTime: s.scheduledUpdateTime,
          updateDuration: s.updateDuration,
          supportDimensions: s.supportDimensions,
          defaultDimension: s.defaultDimension
        };
      t.push(a);
    }
    return t;
  }
  processAbilities(e) {
    if (undefined === e) return;
    const t = [];
    for (const {
      forms: o,
      label: s,
      name: a,
      visible: i
    } of e) {
      const e = {
        name: a,
        label: s,
        visible: i,
        forms: this.processForms(o)
      };
      t.push(e);
    }
    return t;
  }
  outputJSON(e, t, o, s) {
    e.anonymizeDebug("Module Pack Info: ", t), fse.outputJSONSync(path_1.resolve(o, s, build_directory_const_js_1.BuildArtifactConst.PACK_INFO), t);
  }
  getPackInfoObj(e, t, o) {
    return {
      summary: {
        app: e,
        modules: t
      },
      packages: o
    };
  }
  getEntryDeviceType(e, t) {
    return e ? e.getTargetDeviceType() : t.getDeviceTypes();
  }
  getDeviceTypeSubset(e, t, o, s) {
    const a = new Set(s),
      i = o.filter(e => a.has(e));
    return 0 === i.length && e.warn(`The device types of ${this.moduleModel.getName()} and ${t} are different. Invalid HAP ${this.moduleModel.getName()}.`), i;
  }
}
