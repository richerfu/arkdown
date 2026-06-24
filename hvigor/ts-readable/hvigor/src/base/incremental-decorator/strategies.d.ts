/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/strategies.d.ts
 */
import { FileSet, FsOptions } from '../internal/snapshot/util/file-set';
import { TaskInputValue } from '../internal/snapshot/util/task-input-value-entry';
import { IncrementalDecoratorType } from './types';
export interface DecoratorDeclarations {
    inputFiles: FileSet;
    outputFiles: FileSet;
    inputs: Map<string, TaskInputValue>;
    ignoreDecoratorTypes: Set<IncrementalDecoratorType>;
}
type Handler = (propertyKey: string, propertyValue: any, decoratorDeclarations: DecoratorDeclarations, options?: FsOptions) => void;
/**
 * 在收集装饰器装饰的属性时，一些策略处理
 */
export declare const Strategies: Record<IncrementalDecoratorType, Handler>;
export {};
