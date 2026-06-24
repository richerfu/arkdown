/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/sign-job.js
 */
import * as process_utils_js_1 from "../../utils/process-utils";
export const runSign = async s => {
  const e = new process_utils_js_1.ProcessUtils(s.moduleName, `Sign${s.signType}`);
  await e.execute(s.verifySignCommands), await e.execute(s.signCommand);
};
export const signPackageFromApp = async s => {
  const e = new process_utils_js_1.ProcessUtils(s.projectName, `Sign${s.signType}`);
  await e.execute(s.signCommand);
};
