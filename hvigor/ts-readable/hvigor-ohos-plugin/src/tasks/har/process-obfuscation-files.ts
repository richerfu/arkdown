/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/process-obfuscation-files.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import fs_extra_1 from "fs-extra";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as get_obfuscation_rules_js_1 from "../../common/obfuscation/get-obfuscation-rules";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as obfuscation_config_resolver_js_1 from "../../utils/obfuscation-config-resolver";
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as ohos_har_task_js_1 from "../task/ohos-har-task";
export class ProcessObfuscationFiles extends ohos_har_task_js_1.OhosHarTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.PROCESS_OBFUSCATION_FILES), this._log = ohos_logger_js_1.OhosLogger.getLogger(ProcessObfuscationFiles.name);
  }
  taskShouldDo() {
    var e, s, o;
    const t = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.obfuscation;
    return undefined !== (null === (s = this.moduleModel.getSourceSetByTargetName(this.targetName).getCodeMap().get(code_type_enum_js_1.CodeType.ETS)) || undefined === s ? undefined : s.getSrcPath()) && !this._harExtendInfo.isObfuscatedHar() && !!(null == t ? undefined : t.consumerFiles) && (null === (o = t.consumerFiles) || undefined === o ? undefined : o.length) > 0;
  }
  async doTaskAction() {
    var e;
    if (this.compileApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_10 || this.isFaMode) return;
    const s = this.targetService.getBuildOption();
    if (s.debuggable) return;
    s.artifactType && this._log.warn("buildOption/artifactType is deprecated, please use buildOption/arkOptions/obfuscation instead.");
    const o = null === (e = s.arkOptions) || undefined === e ? undefined : e.obfuscation;
    if (!o) return;
    let t;
    if (true === hvigor_1.hvigorCore.getParameter().getPropertyInternal("ohos.obfuscationRules.optimization")) try {
      t = await (0, get_obfuscation_rules_js_1.getObfuscationOptionsWithCache)(this.targetService, o, "");
    } catch (e) {
      t = await (0, obfuscation_config_resolver_js_1.resolveObfuscationOptions)(this.targetService, o, "");
    } else t = await (0, obfuscation_config_resolver_js_1.resolveObfuscationOptions)(this.targetService, o, "");
    t && (await fs_extra_1.ensureDir(path_1.dirname(t.exportRulePath)), await (0, hvigor_arkts_compose_1.generateObfuscationFile)(t, this.sdkInfo.getEtsLoader(), this.sdkInfo.getSdkVersion(), this.sdkInfo.getEtsComponentVersion(), this.isFaMode, this._log));
  }
  initTaskDepends() {
    this.declareDepends(compile_resource_js_1.CompileResource.name);
  }
}
