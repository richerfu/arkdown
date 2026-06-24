/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/module-task-service.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as util from "util";
import * as hvigor_1 from "@ohos/hvigor";
import * as task_util_js_1 from "@ohos/hvigor/src/base/util/task-util";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as hvigor_extra_config_info_js_1 from "../../common/hvigor-extra-config-info";
import * as module_path_info_iml_js_1 from "../../common/iml/module-path-info-iml";
import * as common_const_js_1 from "../../const/common-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as module_target_impl_js_1 from "../../plugin/context/impl/module-target-impl";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as target_utils_js_1 from "../../utils/target-utils";
import * as hap_task_target_data_js_1 from "../data/hap-task-target-data";
import * as status_js_1 from "./status";
import * as task_service_js_1 from "./task-service";
const _log = ohos_logger_js_1.OhosLogger.getLogger("Hvigor-module-service");
export class ModuleTaskService extends task_service_js_1.TaskService {
  constructor(t, e, r, o) {
    super(e.getModule(), t, r, o), this._moduleModel = e, this._targetDataSet = new Set(), this._targets = [], this._targetArtifactNames = new Map(), this._status = new status_js_1.FullStatus(this.initTargetData());
  }
  getModuleModel() {
    return this._moduleModel;
  }
  getTargetDataSet() {
    return this._targetDataSet;
  }
  getTargets() {
    return this._targets;
  }
  getTargetArtifactNames() {
    return this._targetArtifactNames;
  }
  isArkModule() {
    return this._moduleModel.isArkModule();
  }
  getRelatedEntryModules() {
    var t;
    return null !== (t = this._moduleModel.getRelatedEntryModules()) && undefined !== t ? t : undefined;
  }
  static checkEntryModules(t, e) {
    return t === module_type_enum_js_1.ModuleType.Feature && undefined !== e && 0 !== e.length;
  }
  hasLiteDevice() {
    var t;
    return null === (t = this._moduleModel.getDeviceTypes()) || undefined === t ? undefined : t.some(t => "liteWearable" === t || "smartVision" === t);
  }
  initTargetData() {
    const t = this._moduleModel.getTargetOptions(),
      e = target_utils_js_1.TargetUtils.computeTargets(this._projectModel.getModuleSpecificTargets().get(this._moduleModel.getName()), t),
      r = (0, hvigor_extra_config_info_js_1.getBuildRoot)(),
      o = this._moduleModel.getProjectDir();
    path_1.normalize(path_1.relative(o, path_1.resolve(o, r))).startsWith("..") && _log.printErrorExit("INVALID_BUILDROOT_VALUE", [r]);
    const s = (0, find_target_product_js_1.findTargetProduct)(this._projectModel),
      a = [new status_js_1.Status(this._moduleModel.getProfileOpt())];
    return t.forEach(t => {
      var o, i;
      const _ = t.name,
        u = this.targetNeedPack(s, _, e),
        l = new module_path_info_iml_js_1.ModulePathInfoIml(this._moduleModel, _, s.name, r),
        n = new hap_task_target_data_js_1.ModuleTargetData(this._moduleModel, t, l, s);
      u && (this._targets.push(new module_target_impl_js_1.ModuleTargetImpl(t, l, this._projectModel, this._moduleModel)), (null === (o = t.output) || undefined === o ? undefined : o.artifactName) && this._targetArtifactNames.set(_, null === (i = t.output) || undefined === i ? undefined : i.artifactName)), this._targetDataSet.add([n, u]), a.push(n.getTargetStatus());
    }), a;
  }
  findTargetDataByName(t) {
    for (const e of this._targetDataSet.values()) {
      const r = e[0];
      if (r.getTargetName() === t) return r;
    }
  }
  targetNeedPack(t, e, r) {
    let o = false;
    return r.indexOf(e) > -1 && (o = true), r.indexOf("all") > -1 && (o = e !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET), inject_util_js_1.InjectUtil.isDeviceTest() && (o = e === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET || e === common_const_js_1.DefaultTargetConst.DEFAULT_TARGET), inject_util_js_1.InjectUtil.isSync() && !hasCommandEntryTask() && this.hasCppInOhosTest() && (o = e === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET || o), checkHasTargetApplyProduct(this._moduleModel, e, t.name) && o;
  }
  hasCppInOhosTest() {
    return fs_1.existsSync(path_1.resolve(this._moduleModel.getSourceRootByTargetName(common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET), code_type_enum_js_1.CodeType.CPP));
  }
}
function hasCommandEntryTask() {
  const t = hvigor_1.hvigor.getCommandEntryTaskInternal();
  return undefined !== t && t.length > 0;
}
function getProducts(t, e, r, o) {
  if (r === common_const_js_1.CommonConst.OHOS_TEST) return [o];
  const s = null == t ? undefined : t.getTargetApplyProducts(e.getName(), r);
  return null != s ? s : ["default"];
}
export function checkHasTargetApplyProduct(t, e, r) {
  if (t.isHarModule() || (0, task_util_js_1.getAlignTarget)()) return true;
  const o = t.getParentProject(),
    s = getProducts(o, t, e, r),
    a = o.getProductNames();
  return s.some(t => !a.includes(t)) && _log.printErrorExit("INVALID_PRODUCT_FOR_TARGET", [e, o.getProfilePath()], [[e, util.format(s)]]), s.includes(r);
}
