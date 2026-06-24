/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/strategies.js
 */
import fs_extra_1 from "fs-extra";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as types_js_1 from "./types";
import * as util_js_1 from "./util";
const inputHandler = (e, t, {
    inputs: r
  }) => {
    const n = ["number", "string", "boolean"],
      o = null != t ? t : "";
    if (n.includes(typeof o)) r.set(e, o);else if (Array.isArray(o) && o.every(e => n.includes(typeof e))) r.set(e, o);else try {
      r.set(e, (0, hvigor_common_1.hash)(JSON.stringify(o)));
    } catch (t) {
      util_js_1.incrementalDeclaratorLogger.debug(`Error occurs while handling @Input '${e}': ${(null == t ? undefined : t.message) || ""}`);
    }
  },
  inputsHandler = (e, t, r) => {
    if (t instanceof Map) for (const [n, o] of t) inputHandler(`${e}-${n}`, o, r);else util_js_1.incrementalDeclaratorLogger.debug(`Decorator @Inputs can only be used for 'Map', but get '${(0, util_js_1.getType)(t)}' from '${e}'.`);
  },
  inputFileHandler = (e, t, {
    inputFiles: r
  }, n) => {
    "string" == typeof t ? "" !== t && fs_extra_1.existsSync(t) ? r.addEntry(t, n ? {
      ...n
    } : undefined) : util_js_1.incrementalDeclaratorLogger.debug(`File: '${t}' from '${e}' is not exists, just ignore.`) : util_js_1.incrementalDeclaratorLogger.debug(`Decorator @InputFile can only be used for 'String', but get '${(0, util_js_1.getType)(t)}' from '${e}'.`);
  },
  inputFilesHandler = (e, t, r, n) => {
    Array.isArray(t) ? t.forEach(t => {
      inputFileHandler(e, t, r, n);
    }) : util_js_1.incrementalDeclaratorLogger.debug(`Decorator @InputFiles can only be used for 'Array', but get '${(0, util_js_1.getType)(t)}' from '${e}'.`);
  },
  outputFileHandler = (e, t, {
    outputFiles: r
  }, n) => {
    "string" == typeof t ? "" !== t ? r.addEntry(t, n ? {
      ...n
    } : undefined) : util_js_1.incrementalDeclaratorLogger.debug(`Decorator @OutputFile get empty string from '${e}', just ignore.`) : util_js_1.incrementalDeclaratorLogger.debug(`Decorator @OutputFile can only be used for 'String', but get '${(0, util_js_1.getType)(t)}' from '${e}'.`);
  },
  outputFilesHandler = (e, t, r, n) => {
    Array.isArray(t) ? t.forEach(t => {
      outputFileHandler(e, t, r, n);
    }) : util_js_1.incrementalDeclaratorLogger.debug(`Decorator @OutputFiles can only be used for 'Array', but get '${(0, util_js_1.getType)(t)}' from '${e}'.`);
  },
  ignoreSuperIncrementalHandler = (e, t, {
    ignoreDecoratorTypes: r
  }) => {
    const n = {
        declareInputs: [types_js_1.IncrementalDecoratorType.Input, types_js_1.IncrementalDecoratorType.Inputs],
        declareInputFiles: [types_js_1.IncrementalDecoratorType.InputFile, types_js_1.IncrementalDecoratorType.InputFiles],
        declareOutputFiles: [types_js_1.IncrementalDecoratorType.OutputFile, types_js_1.IncrementalDecoratorType.OutputFiles]
      },
      o = Object.keys(n);
    o.includes(e) ? n[e].forEach(e => {
      r.add(e);
    }) : util_js_1.incrementalDeclaratorLogger.debug(`Decorator @IgnoreSuperIncremental can only be used for '${o.join(", ")}'`);
  };
export const Strategies = {
  [types_js_1.IncrementalDecoratorType.Input]: inputHandler,
  [types_js_1.IncrementalDecoratorType.Inputs]: inputsHandler,
  [types_js_1.IncrementalDecoratorType.InputFile]: inputFileHandler,
  [types_js_1.IncrementalDecoratorType.InputFiles]: inputFilesHandler,
  [types_js_1.IncrementalDecoratorType.OutputFile]: outputFileHandler,
  [types_js_1.IncrementalDecoratorType.OutputFiles]: outputFilesHandler,
  [types_js_1.IncrementalDecoratorType.IgnoreSuperIncremental]: ignoreSuperIncrementalHandler
};
