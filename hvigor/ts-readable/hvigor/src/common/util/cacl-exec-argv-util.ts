/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/cacl-exec-argv-util.js
 */
import process_1 from "process";
import * as hvigor_log_js_1 from "../../base/log/hvigor-log";
import * as hvigor_config_reader_js_1 from "../../base/util/hvigor-config-reader";
import * as const_js_1 from "../const/const";
const logger = hvigor_log_js_1.HvigorLogger.getLogger(hvigor_config_reader_js_1.HvigorConfigReader.name);
export function calcChildExecArgv(e = false) {
  var r, o;
  const s = [...process_1.execArgv],
    i = `--max-old-space-size=${hvigor_config_reader_js_1.HvigorConfigReader.getMaxOldSpaceSize(e)}`,
    _ = s.findIndex(e => e.startsWith("--max-old-space-size="));
  -1 !== _ && s[_] ? s[_] = i : s.push(i);
  const c = `--max-semi-space-size=${hvigor_config_reader_js_1.HvigorConfigReader.getMaxSemiSpaceSize(e)}`,
    a = s.findIndex(e => e.startsWith("--max-semi-space-size="));
  -1 !== a && s[a] ? s[a] = c : s.push(c);
  const g = null === (o = null === (r = hvigor_config_reader_js_1.HvigorConfigReader.getHvigorConfig()) || undefined === r ? undefined : r.nodeOptions) || undefined === o ? undefined : o.exposeGC,
    t = s.indexOf("--expose-gc");
  false === g || -1 !== t && s[t] ? false !== g || -1 === t && !s[t] || s.splice(t, 1) : s.push("--expose-gc");
  const n = hvigor_config_reader_js_1.HvigorConfigReader.getStacktrace(e);
  return s.some(e => e === const_js_1.ENABLE_SOURCE_MAPS) ? !n && logger.warn(`"${const_js_1.ENABLE_SOURCE_MAPS}" is enabled but has no effect because "stacktrace" is disabled. Please enable "stacktrace" first.`) : n && s.push(const_js_1.ENABLE_SOURCE_MAPS), s;
}
