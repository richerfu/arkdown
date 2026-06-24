/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/sync.d.ts
 */
import { HvigorCoreNode } from '../../external/core/hvigor-core-node';
/**
 * 执行每个module中注册的sync类型的任务，将对应的数据模型注册到registry中
 *
 * @param {HvigorCoreNode} hvigorNode
 */
export declare function findSyncTaskPathsForNode(hvigorNode: HvigorCoreNode): string[];
/**
 * 打印sync的信息到输出流中
 */
export declare function outputPluginSyncInfo(): void;
