/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/base/base-sign.js
 */
import * as sign_type_enum_js_1 from "../../enum/sign-type-enum";
import * as sign_util_js_1 from "../sign/sign-util";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class BaseSign extends ohos_hap_task_js_1.OhosHapTask {
  constructor(s, i, n) {
    super(s, i), this._log = ohos_logger_js_1.OhosLogger.getLoggerWithDurationId(BaseSign.name, this.durationEvent.getId()), this.signType = null != n ? n : this.targetData.isHarmonyOS() ? sign_type_enum_js_1.SignTypeEnum.HOS_HAP : sign_type_enum_js_1.SignTypeEnum.HAP, this.signUtil = new sign_util_js_1.SignUtil(this.service, this.signType, this.pathInfo, this.targetData.getProduct(), this.sdkInfo), this.signingConfig = this.signUtil._signingConfig;
  }
  async beforeAlwaysAction() {
    this._log.warn(this.signUtil._signingConfigCheckLogStr);
  }
  declareInputs() {
    return sign_util_js_1.SignUtil.getSigningConfigInputs(this.sdkInfo, this.signingConfig);
  }
  declareInputFiles() {
    return this.signUtil.getSigningConfigInputFiles();
  }
}
