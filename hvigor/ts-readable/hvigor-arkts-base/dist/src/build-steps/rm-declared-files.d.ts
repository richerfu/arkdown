/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/rm-declared-files.d.ts
 */
import { RollupCache } from 'rollup';
import { HvigorOptions } from '../options/hvigor-options';
/**
 * 基于rollup缓存收集上一次参与编译的源码文件
 *
 * @param options
 */
export declare function getModuleLastCachedSourceFiles(options: HvigorOptions): string[];
/**
 * 移除掉多余的声明文件
 * @param options
 * @param cachedSourceFiles
 * @param declaredFilesPath
 * @param modulePath
 * @param cache
 */
export declare function removeRedundantDeclaredFiles(options: HvigorOptions, cachedSourceFiles: string[], declaredFilesPath?: string, modulePath?: string, cache?: RollupCache | false): Promise<void>;
