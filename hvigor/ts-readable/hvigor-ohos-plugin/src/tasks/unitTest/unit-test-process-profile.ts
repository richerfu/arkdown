/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/unit-test-process-profile.js
 */
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as fse from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as json_util_js_1 from "../../utils/json-util";
import * as process_profile_js_1 from "../process-profile";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class UnitTestProcessProfile extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, CommonTask.UNIT_TEST_PROCESS_PROFILE), this.processProfileModuleJsonPath = this.pathInfo.getIntermediatesProcessProfile();
  }
  initTaskDepends() {
    this.declareDepends(process_profile_js_1.ProcessProfile.name);
  }
  doTaskAction() {
    var e;
    const t = path_1.resolve(this.pathInfo.getModuleBuildPath(), common_const_js_1.UnitTestConst.TEST_ABILITY_ETS_PATH);
    if (!fse.existsSync(t)) return;
    const s = json_util_js_1.JsonUtil.getJson5Obj(this.processProfileModuleJsonPath);
    let o = null === (e = null == s ? undefined : s.module) || undefined === e ? undefined : e.abilities;
    const n = this.targetService.getModuleService().getModuleModel();
    (null == o ? undefined : o.length) || (o = s.module.abilities = []), (n.isHspModule() || n.isHarModule()) && (s.module.deliveryWithInstall = n.isHarModule(), n.isHarModule() && (s.module.type = "entry", s.app.icon = common_const_js_1.CommonConst.ICON, s.app.label = common_const_js_1.CommonConst.LABEL));
    const {
        startWindowIcon: r,
        startWindowBackground: i
      } = (null == o ? undefined : o[0]) || {
        startWindowIcon: "startIcon",
        startWindowBackground: "#FFFFFF"
      },
      a = {
        name: common_const_js_1.UnitTestConst.TEST_ABILITY,
        srcEntrance: common_const_js_1.UnitTestConst.ENTRY_TEST_ABILITY_PATH,
        startWindowIcon: r,
        startWindowBackground: i
      };
    o.push(a), fse.outputJSONSync(this.processProfileModuleJsonPath, s, {
      spaces: "\t"
    }), Object.assign(s.module, {
      pages: "$profile:main_pages"
    }), fse.outputJSONSync(this.pathInfo.getIntermediatesArkModuleJsonPath(), s, {
      spaces: "\t"
    });
  }
}
