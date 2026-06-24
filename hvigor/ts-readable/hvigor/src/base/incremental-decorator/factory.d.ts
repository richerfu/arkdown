/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/factory.d.ts
 */
import { IncrementalTask } from '../external/task/incremental-task';
import { FsOptions } from '../internal/snapshot/util/file-set';
import { IncrementalDecoratorType } from './types';
type IncrementalDecorator = (target: IncrementalTask, propertyKey: string) => void;
/**
 * 用于生成Input/Inputs/IgnoreSuperIncremental装饰器
 * @param declaratorType
 */
export declare const createBaseDecorator: (declaratorType: IncrementalDecoratorType) => (() => IncrementalDecorator);
/**
 * 用于生成InputFile/InputFiles/OutputFile/OutputFiles装饰器
 * @param declaratorType
 */
export declare const createFileDecorator: (declaratorType: IncrementalDecoratorType) => (options?: FsOptions) => IncrementalDecorator;
export {};
