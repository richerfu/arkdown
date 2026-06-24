/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/buildInfo-cache-service.d.ts
 */
import { DefaultCacheService } from './default-cache-service';
import { Snapshot } from '../../snapshot/core/snapshot';
import { BuildInfoCacheEntry } from '../entry/buildInfo-cache-entry';
/**
 * 提供获取构建信息缓存快照的统一服务接口
 *
 *
 */
export declare class BuildInfoCacheService extends DefaultCacheService<Snapshot> {
    private readonly _buildInfoCachePath;
    constructor(buildInfoCachePath: string);
    set(key: string, entryContent: Snapshot): BuildInfoCacheEntry;
    initialize(): void;
}
