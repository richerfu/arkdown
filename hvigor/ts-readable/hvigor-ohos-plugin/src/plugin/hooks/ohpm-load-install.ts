/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/ohpm-load-install.js
 */
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as hvigor_1 from "@ohos/hvigor";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as path_const_js_1 from "@ohos/hvigor/src/common/const/path-const";
import * as common_const_js_1 from "../../const/common-const";
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as json_util_js_1 from "../../utils/json-util";
import * as run_command_util_js_1 from "../../utils/run-command-util";
import fs_1 from "fs";
import * as global_project_data_service_js_1 from "../../tasks/service/global-project-data-service";
import * as common_util_1 from "../../common/common-util";
export class OhpmLoadInstallService {
  constructor() {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(OhpmLoadInstallService.name);
  }
  ohpmLoadInstall(o) {
    var e;
    const t = null !== (e = (0, hvigor_1.getAlignTarget)()) && undefined !== e ? e : "";
    this.isOnlyCleanTask() || this.ohpmLoadInstallForBuild(t);
    const a = global_project_data_service_js_1.GlobalProjectDataService.getInstance().initProjectPkgJsonFileHash(o.getProjectModel());
    global_project_data_service_js_1.GlobalProjectDataService.getInstance().setProjectPkgJsonFileHash(a);
  }
  ohpmLoadInstallForBuild(o) {
    const e = path_1.resolve(path_const_js_1.HVIGOR_PROJECT_ROOT_DIR, common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
    let t;
    if (fs_extra_1.existsSync(e)) {
      const o = json_util_js_1.JsonUtil.getJson5Obj(e),
        a = hvigor_1.hvigorCore.getParameter().getExtParamInternal(common_const_js_1.CommonConst.PARAMETER_FILE);
      t = (0, common_util_1.replaceStringParam)(null != a ? a : null == o ? undefined : o.parameterFile, o.properties);
    }
    if (oh_package_loader_js_1.ohPackageLoader.setIsChangeParameterFile(o, t), oh_package_loader_js_1.ohPackageLoader.loadUpdatedOhPackageToDisk(o, t), oh_package_loader_js_1.ohPackageLoader.shouldDoOhpmInstall()) {
      this._log.info("start to execute ohpm install");
      const e = process.env.ohpmBin || "ohpm";
      if (e && fs_1.existsSync(e) || run_command_util_js_1.RunCommandUtil.run("ohpm -v", undefined, true)) {
        const a = path_1.resolve(hvigor_1.PathUtil.getHvigorCacheDir(), common_const_js_1.CommonConst.DEPENDENCYMAP, o);
        let s;
        this._log.debug(`ohpm install command is "${e}" i --all --target_path ${a}`), s = t ? `"${e}" i --all --target_path ${a} --parameterFile=${t}` : `"${e}" i --all --target_path ${a}`;
        run_command_util_js_1.RunCommandUtil.run(s, undefined, true) || this._log.printErrorExit("OHPM_INSTALL_FAILED");
      } else this._log.warn("need to configure ohpm environment in your system, please see official guide to configure ohpm environment");
    }
  }
  isOnlyCleanTask() {
    const o = hvigor_1.hvigor.getCommandEntryTaskInternal();
    return 1 === (null == o ? undefined : o.length) && "clean" === o[0];
  }
}
