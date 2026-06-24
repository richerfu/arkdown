/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/watch.d.ts
 */
/// <reference types="node" />
import { MessagePort } from 'worker_threads';
import { RollupWatcher } from 'rollup';
import { LogEvent } from '../log/arkts-base-log';
import { HvigorOptions } from '../options/hvigor-options';
import { BuildResult } from '../util/build-result';
/**
 * 启动watch监听
 *
 * @param options
 * @param isPreview
 * @param handleLogs
 * @param parentPort
 * @param initServerOnce
 * @param setWatcher
 * @param enableFileSystemCache
 * @param cacheDirectory
 * @param unsupportedChangedFiles
 * @param printError
 */
export declare function startWatch(options: HvigorOptions, isPreview: boolean, handleLogs: (logs: LogEvent[], lastReUsedFiles?: Set<string>) => void, parentPort: MessagePort, initServerOnce: () => void, setWatcher: (w: RollupWatcher) => void, enableFileSystemCache: boolean | undefined, cacheDirectory: string | undefined, unsupportedChangedFiles: string[], printError?: (error: any) => void): Promise<BuildResult>;
/**
 * 写watch缓存
 *
 * @param pkgName2SourceRoots
 */
export declare function writeWatchCache(pkgName2SourceRoots?: Record<string, string[]>): Promise<void>;
