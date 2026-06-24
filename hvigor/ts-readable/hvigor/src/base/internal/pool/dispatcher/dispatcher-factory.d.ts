/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/dispatcher/dispatcher-factory.d.ts
 */
import { DispatcherType } from '../enum/dispatcher-type';
import { Dispatcher } from './dispatcher';
/**
 * 分发器工厂
 * 创建不同类型的分发器
 *
 * @since 2022/8/16
 */
export declare class DispatcherFactory {
    static getDispatcher(type: DispatcherType): Dispatcher | undefined;
}
