/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build.d.ts
 */
/// <reference types="node" />
import { MessagePort } from 'worker_threads';
import { RollupWatcher } from 'rollup';
import { LogEvent } from './log/arkts-base-log';
import { OhosLogger } from './log/ohos-logger';
import { HvigorOptions } from './options/hvigor-options';
import { BuildResult } from './util/build-result';
import { CacheStoreManager } from './util/cache-store-type';
export declare function build(options: HvigorOptions, logger: OhosLogger, handleLogs?: (logs: LogEvent[], lastReUsedFiles?: Set<string>) => void, parentPort?: MessagePort, initServerOnce?: () => void, setWatcher?: (w: RollupWatcher) => void, cacheStoreManager?: CacheStoreManager): Promise<BuildResult>;
