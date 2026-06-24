/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/cpu-abi-enum.js
 */
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("CpuAbiEnum");
export var CpuAbiEnum;
!function (o) {
  o.ARM64_V8A = "aarch64-linux-ohos", o.ARMEABI_V7A = "arm-linux-ohos", o.X86_64 = "x86_64-linux-ohos";
}(CpuAbiEnum = CpuAbiEnum || (CpuAbiEnum = {})), function (o) {
  o.getCpuType = u => {
    switch (u) {
      case "arm64-v8a":
        return o.ARM64_V8A;
      case "armeabi-v7a":
        return o.ARMEABI_V7A;
      case "x86_64":
        return o.X86_64;
      default:
        return _log.printErrorExit("UNKNOWN_CPU_ABI", [u]), o.ARM64_V8A;
    }
  };
}(CpuAbiEnum = CpuAbiEnum || (CpuAbiEnum = {}));
