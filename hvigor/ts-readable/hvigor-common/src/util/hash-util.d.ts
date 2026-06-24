/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/hash-util.d.ts
 */
import crypto, { BinaryLike } from 'crypto';
export declare const createHash: (algorithm?: string) => crypto.Hash;
export declare const hash: (data: BinaryLike, algorithm?: string) => string;
export declare const hashFile: (filePath: string, algorithm?: string) => string | undefined;
/**
 * 以安全方式获取对象的json字符串，不会抛出异常，用于替换JSON.stringIfy()函数（会忽略对象中的函数、bigint、symbol数据类型）
 * 此函数不会产生异常
 * @param obj
 */
export declare const getSafeJsonStringIfy: (obj: any) => string;
/**
 * 采用sha256算法计算对象的hash值（会忽略对象中的函数、bigint、symbol数据类型）
 * 此函数不会产生异常
 * @param obj
 */
export declare const hashObject: (obj: any) => string;
