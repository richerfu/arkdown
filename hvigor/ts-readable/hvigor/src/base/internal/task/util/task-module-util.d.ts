/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/util/task-module-util.d.ts
 */
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
/**
 * 通过Task的path分割获取其所属Module
 *
 * @param {string} taskPath
 * @returns {HvigorNode | undefined}
 */
export declare function getModuleFromTaskPath(taskPath: string): HvigorCoreNode | undefined;
