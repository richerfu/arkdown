/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/process-importee.d.ts
 */
import { RollupCache } from 'rollup';
import { HvigorOptions } from '../options/hvigor-options';
/**
 * 组织收集nonEntryImportee和compiledImportee
 *
 * @param options
 * @param arkOpt
 * @param modulePath
 * @param cacheDir
 * @param cache
 */
export declare function processImportees(options: HvigorOptions, arkOpt: {
    isByteCodeHar?: boolean;
    bundledDependencies?: boolean;
    shouldCollectDirectImportees?: boolean;
}, modulePath?: string, cacheDir?: string, cache?: RollupCache): Promise<void>;
