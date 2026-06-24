/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/class-identify-util.d.ts
 */
import { Class } from '../external/core/hvigor-core-node';
/**
 * 判断给定的obj对象是否是某个对应的classType
 *
 * @param {Class} obj
 * @param {string} classType
 * @returns {boolean}
 */
export declare function instanceOf<T extends Class>(obj: T, classType: string): boolean;
