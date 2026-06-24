/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/sign-command-factory.js
 */
import * as har_signing_options_js_1 from "../../../builder/inner-java-command-builder/har-signing-options";
import * as signing_options_v2_1 from "../../../builder/inner-java-command-builder/signing-options-v2";
import * as signing_options_js_1 from "../../../builder/inner-java-command-builder/signing-options";
import * as sign_type_enum_js_1 from "../../../enum/sign-type-enum";
import * as bin_sign_command_builder_js_1 from "./bin-sign-command-builder";
import * as hap_sign_command_builder_1 from "./hap-sign-command-builder";
import * as hapv2_sign_command_builder_1 from "./hapv2-sign-command-builder";
import * as har_sign_command_builder_1 from "./har-sign-command-builder";
export class SignCommandFactory {
  constructor(n) {
    this._compileSdkVersion = n;
  }
  createCommandBuilder(n, i, e, _) {
    switch (_.getSignType()) {
      case sign_type_enum_js_1.SignTypeEnum.HOS_HAP:
        return this._compileSdkVersion > 8 ? new hap_sign_command_builder_1.HapSignCommandBuilder(n, i, e, _, new signing_options_v2_1.SigningOptionsV2()) : new hapv2_sign_command_builder_1.Hapv2SignCommandBuilder(n, i, e, _, new signing_options_js_1.SigningOptions());
      case sign_type_enum_js_1.SignTypeEnum.APP:
        return "HarmonyOS" === i.type && this._compileSdkVersion < 9 ? new hapv2_sign_command_builder_1.Hapv2SignCommandBuilder(n, i, e, _, new signing_options_js_1.SigningOptions()) : new hap_sign_command_builder_1.HapSignCommandBuilder(n, i, e, _, new signing_options_v2_1.SigningOptionsV2());
      case sign_type_enum_js_1.SignTypeEnum.BIN:
        return new bin_sign_command_builder_js_1.BinSignCommandBuilder(n, i, e, _, new signing_options_v2_1.SigningOptionsV2());
      case sign_type_enum_js_1.SignTypeEnum.HAR:
        return new har_sign_command_builder_1.HarSignCommandBuilder(n, i, e, _, new har_signing_options_js_1.HarSigningOptions());
    }
    return this._compileSdkVersion < 8 ? new hapv2_sign_command_builder_1.Hapv2SignCommandBuilder(n, i, e, _, new signing_options_js_1.SigningOptions()) : new hap_sign_command_builder_1.HapSignCommandBuilder(n, i, e, _, new signing_options_v2_1.SigningOptionsV2());
  }
}
