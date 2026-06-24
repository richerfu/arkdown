/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/job.d.ts
 */
import { CacheStoreManager } from '@ohos/hvigor';
import { CompileEvent, ProjectConfig } from '@ohos/hvigor-arkts-compose';
export declare const run: (config: ProjectConfig, cacheStoreManager?: CacheStoreManager) => Promise<CompileEvent[] | Error>;
export declare const runArkLint: (config: {
    config: ProjectConfig;
    entry: Record<string, unknown>;
}) => Promise<void>;
