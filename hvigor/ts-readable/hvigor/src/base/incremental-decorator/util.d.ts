/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/util.d.ts
 */
import { IncrementalTask } from '../external/task/incremental-task';
import { FsOptions } from '../internal/snapshot/util/file-set';
import { HvigorLogger } from '../log/hvigor-log';
import { IncrementalDecoratorType } from './types';
export declare const incrementalDeclaratorLogger: HvigorLogger;
interface DecoratorContainerMember {
    propertyKey: string;
    declaratorType: IncrementalDecoratorType;
    options?: FsOptions;
}
/**
 * 在target对象上，设置一个不可枚举的属性。
 * 其中。key为Symbol，可以保证全局唯一，不会存在属性名冲突
 * 值为数组，用于储存装饰器装饰的属性名。装饰器类型，以及后续所需的额外参数（FsOption）
 * @param target
 */
export declare const setSelfDeclaratorContainer: (target: IncrementalTask) => void;
/**
 * 获取自身的基于装饰器装饰的属性
 * @param target
 */
export declare const getSelfDeclaratorContainer: (target: IncrementalTask) => undefined | DecoratorContainerMember[];
/**
 * 判断当前任务是否存在增量装饰器装饰的属性
 * @param target
 */
export declare const hasIncrementalDecorator: (target: IncrementalTask) => boolean;
/**
 * _toString.call(’xxx‘)会返回 [object String]之类的，用此方式可以精准获取入参的类型
 * @param param
 */
export declare const getType: (param: any) => string;
export {};
