/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/index.js
 */
import * as factory_js_1 from "./factory";
import * as types_js_1 from "./types";
export const Input = (0, factory_js_1.createBaseDecorator)(types_js_1.IncrementalDecoratorType.Input);
export const Inputs = (0, factory_js_1.createBaseDecorator)(types_js_1.IncrementalDecoratorType.Inputs);
export const InputFile = (0, factory_js_1.createFileDecorator)(types_js_1.IncrementalDecoratorType.InputFile);
export const InputFiles = (0, factory_js_1.createFileDecorator)(types_js_1.IncrementalDecoratorType.InputFiles);
export const OutputFile = (0, factory_js_1.createFileDecorator)(types_js_1.IncrementalDecoratorType.OutputFile);
export const OutputFiles = (0, factory_js_1.createFileDecorator)(types_js_1.IncrementalDecoratorType.OutputFiles);
export const IgnoreSuperIncremental = (0, factory_js_1.createBaseDecorator)(types_js_1.IncrementalDecoratorType.IgnoreSuperIncremental);
