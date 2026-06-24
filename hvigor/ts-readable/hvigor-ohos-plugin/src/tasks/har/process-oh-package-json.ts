/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/process-oh-package-json.js
 */
import * as fse from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as task_names_js_1 from "../common/task-names";
import * as core_process_package_json_js_1 from "./core-process-package-json";
import * as har_extend_info_js_1 from "./har-extend-info";
import * as file_util_js_1 from "../../utils/file-util";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as dependency_module_version_validate_1 from "../../validate/dependency-module-version-validate";
var Task = task_names_js_1.TaskNames.Task;
export class ProcessOhPackageJson extends core_process_package_json_js_1.CoreProcessPackageJson {
  constructor(e) {
    super(e, Task.PROCESS_OH_PACKAGE_JSON), this.logger = ohos_logger_js_1.OhosLogger.getLogger(ProcessOhPackageJson.name), this.generateOhPackageJsonFilePath = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(this.generatePmDir, common_const_js_1.CommonConst.OH_PACKAGE_JSON5)), this.workerConfig = this.processWorkerConfig();
  }
  doTaskAction() {
    super.doTaskAction(), this.addWorkerData(), this.addDynamicImport(), this.processByteCodeHar(), processResourceConfig(this.targetService, this.packageJsonObj), this.processInterfaceHar(), (0, dependency_module_version_validate_1.resolveJsonObjWithoutAtModule)(this.packageJsonObj, this.projectModel.getProjectDir()), fse.outputJSONSync(this.generateOhPackageJsonFilePath, this.packageJsonObj);
  }
  addArtifactType(e) {
    e.artifactType = har_extend_info_js_1.ArtifactType.OBFUSCATION;
  }
  processByteCodeHar() {
    if (!this.targetService.isByteCodeHar()) return;
    const e = this.packageJsonObj;
    e.metadata || (e.metadata = {}), e.metadata.byteCodeHar = true, delete e.main;
  }
  processInterfaceHar() {
    const e = this.packageJsonObj;
    "InterfaceHar" === (null == e ? undefined : e.packageType) && this.logger.warn(`"packageType": "InterfaceHar" in oh-package.json5 of har module '${this.moduleModel.getModule().getName()}' is not legal.`);
  }
  addWorkerData() {
    var e;
    this.workerConfig && (this.packageJsonObj.metadata = {
      ...(null !== (e = this.packageJsonObj.metadata) && undefined !== e ? e : {}),
      workers: this.workerConfig
    });
  }
  processWorkerConfig() {
    var e;
    const t = null === (e = this.targetService.getBuildOption().sourceOption) || undefined === e ? undefined : e.workers;
    if ((this.targetService.isByteCodeHar() || this._harExtendInfo.isObfuscatedHar()) && t) {
      const e = this.hasUseTsHarField() ? ".ts" : ".js";
      return t.map(t => {
        const s = path_1.extname(t);
        return `${t.substring(0, t.lastIndexOf(s))}${e}`;
      });
    }
    return t;
  }
  addDynamicImport() {
    var e, t, s;
    const o = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.runtimeOnly;
    if (!o) return;
    const a = this.hasUseTsHarField() ? ".ts" : ".js",
      r = [];
    o.sources && o.sources.forEach(e => {
      if (common_const_js_1.ValidateRegExp.RELATIVE_PATH_REG_EXP.test(e)) {
        const t = path_1.resolve(this.moduleModel.getModule().getNodeDir(), e),
          s = fse.statSync(t);
        s.isDirectory() && r.push(e), s.isFile() && (this.targetService.isOpenSource() || common_const_js_1.ValidateRegExp.JS_FILE_SUFFIX_REG_EXP.test(file_util_js_1.FileUtil.getFileSuffix(e)) ? r.push(e) : r.push(`${e.substring(0, e.lastIndexOf("."))}${a}`));
      }
    });
    let i = [];
    o.packages && (i = o.packages), this.packageJsonObj.metadata = {
      ...(null !== (s = this.packageJsonObj.metadata) && undefined !== s ? s : {}),
      runtimeOnly: {
        sources: r,
        packages: i
      }
    };
  }
}
export function processResourceConfig(e, t) {
  var s, o;
  let a = null === (s = e.getTargetData().getTargetOpt().resource) || undefined === s ? undefined : s.directories;
  if (a && a.length && (a = a.filter(e => !path_1.isAbsolute(e)), a.length)) {
    const e = {
      directories: a
    };
    t.metadata = {
      ...(null !== (o = t.metadata) && undefined !== o ? o : {}),
      resource: e
    };
  }
}
