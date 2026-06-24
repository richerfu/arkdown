/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/daemon-cache-service.d.ts
 */
import { CacheService } from '../../internal/cache/service/cache-service';
import { HvigorLogger } from '../../log/hvigor-log';
import { DaemonInfo } from './daemon-info';
export declare class DaemonCacheService implements CacheService<DaemonInfo> {
    private _log;
    private readonly cachePath;
    constructor(cachePath: string, log: HvigorLogger);
    set(key: string, entryContent: DaemonInfo): void;
    close(): void;
    get(key: string): DaemonInfo;
    remove(key: string): void;
    size(): number;
    initialize(): void;
    getAll(): {
        [keyId: string]: DaemonInfo;
    };
    private removeUselessInfoInDaemonRegistry;
}
