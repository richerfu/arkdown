/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/arkts-pack.d.ts
 */
/// <reference types="node" />
import { MessagePort } from 'worker_threads';
import { CacheStoreManager, LogEvent } from '@ohos/hvigor-arkts-base';
import { CompileEvent } from './analyzer/compile-event';
import { ExtendedErrorInfo } from './data/compose-error';
import { ProjectConfig } from './project-config/project-config';
import { OhosLogger } from './util/log';
export interface ArktsCompileResult {
    compileLogEvents: LogEvent[];
    compileEvents: CompileEvent[];
    isSuccess: boolean;
    error?: Error | ExtendedErrorInfo;
}
/**
 * runArkPack 对外接口
 */
export declare function runArkPack(projectConfig: ProjectConfig, logger: OhosLogger, handleLogs?: (logs: LogEvent[], lastReUsedFiles?: Set<string>) => void, parentPort?: MessagePort, cacheStoreManager?: CacheStoreManager): Promise<ArktsCompileResult>;
export declare function getResolveModules(projectConfig: ProjectConfig): string[];
/**
 * read loader/loader.json
 */
export declare function loadAceBuildJson(aceBuildJsonPath: string | undefined): any;
export { compileCodeSnippet } from './work-scripts/compile-code-snippet';
export declare function watch(projectConfig: ProjectConfig, logger: OhosLogger, handleLogs: (logs: LogEvent[], lastReUsedFiles?: Set<string>) => void, parentPort?: MessagePort): Promise<ArktsCompileResult>;
