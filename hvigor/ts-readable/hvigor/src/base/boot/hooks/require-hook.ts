/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/boot/hooks/require-hook.js
 */
import fs_1 from "fs";
import module_1 from "module";
import * as path from "path";
import os_1 from "os";
import typescript_1 from "typescript";
import * as global_core_parameters_js_1 from "../../internal/data/global-core-parameters";
import * as ts_check_js_1 from "../../internal/util/ts_check";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as compile_options_js_1 from "../../util/options/compile-options";
const log = hvigor_log_js_1.HvigorLogger.getLogger("require_hook"),
  to = {
    compilerOptions: compile_options_js_1.compilerOptions,
    reportDiagnostics: true
  };
function extensionHandler(e, t) {
  const r = path.resolve(__dirname, t),
    o = fs_1.readFileSync(r, "utf-8");
  to.compilerOptions.sourceMap = global_core_parameters_js_1.coreParameter.startParams.printStackTrace;
  const i = typescript_1.transpileModule(o, {
    ...to,
    fileName: t
  });
  handleDiagnostics(r, i.diagnostics);
  let n = i.outputText;
  i.sourceMapText && (n += `${os_1.EOL}//# sourceMappingURL=data:application/json;base64,${Buffer.from(i.sourceMapText).toString("base64")}`), e._compile(n, t);
}
function handleDiagnostics(e, t) {
  const r = {
    getCanonicalFileName: e => e,
    getCurrentDirectory: typescript_1.sys.getCurrentDirectory,
    getNewLine: () => typescript_1.sys.newLine
  };
  null == t || t.forEach(t => {
    log.error((0, ts_check_js_1.formatDiagnostic)(t, r, e));
  }), (null == t ? undefined : t.length) && log.errorMessageExit("Please fix above syntax errors.");
}
export function addExtensionHandler(e) {
  for (const t of e) module_1._extensions[t] = extensionHandler;
}
addExtensionHandler([".ts", ".mjs"]);
