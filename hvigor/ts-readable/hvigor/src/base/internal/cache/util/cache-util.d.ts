/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/util/cache-util.d.ts
 */
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
/**
 * 提供保存任务缓存快照时一个全局唯一的key
 *
 * @param {string} taskName
 * @param {HvigorNode} node
 * @return {string}
 */
export declare function getTaskSnapCacheEntryUniqueKey(taskName: string, node: HvigorCoreNode): string;
