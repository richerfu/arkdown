/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/options/compile-options.js
 */
import path_1 from "path";
import * as typescript_1 from "typescript";
import * as util_js_1 from "../../../cli/wrapper/util";
export const compilerOptions = {
  baseUrl: path_1.resolve((0, util_js_1.getHvigorProjectHome)(), "node_modules"),
  target: typescript_1.ScriptTarget.ES2016,
  module: typescript_1.ModuleKind.CommonJS,
  types: [],
  allowJs: true,
  esModuleInterop: true,
  forceConsistentCasingInFileNames: true,
  strict: true,
  skipLibCheck: true,
  noImplicitAny: true,
  noImplicitReturns: true,
  alwaysStrict: true,
  declaration: true,
  noEmit: true
};
