/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/worker-process-script.js
 */
import * as log_combine_type_js_1 from "../../utils/log/log-combine-type";
import * as process_utils_js_1 from "../../utils/process-utils";
export async function warmUp() {}
export async function generalExecute(e) {
  const {
      moduleName: s,
      taskName: t,
      commandLine: o,
      commandOptions: a,
      logCompileType: r,
      logLevel: c,
      integratedHspCommand: n
    } = e,
    i = new process_utils_js_1.ProcessUtils(s, t, c);
  await i.execute(o, a, r), n && (await i.execute(n));
}
export async function packageHar(e) {
  const {
    moduleName: s,
    taskName: t,
    commandLine: o,
    commandOptions: a,
    logLevel: r
  } = e;
  return (await new process_utils_js_1.ProcessUtils(s, t, r).execute(o, a, log_combine_type_js_1.LogCombineType.DEBUG)).stdout.toString();
}
