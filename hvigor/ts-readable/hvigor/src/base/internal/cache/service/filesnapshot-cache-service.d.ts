/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/filesnapshot-cache-service.d.ts
 */
import { BasicFileSnapshot } from '../../snapshot/core/filesystem-snapshot';
import { FileSnapShotCacheEntry } from '../entry/filesnapshot-cache-entry';
import { DefaultCacheService } from './default-cache-service';
/**
 * 提供获取文件缓存快照的统一服务接口
 *
 * @since 2022/9/1
 */
export declare class FileSnapShotCacheService extends DefaultCacheService<BasicFileSnapshot> {
    private readonly _fileSnapShotCachePath;
    constructor(fileSnapShotCachePath: string);
    initialize(): void;
    /**
     * 需要重载,将对象进行包装
     *
     * @param {string} key
     * @param {BasicFileSnapshot} entryContent
     * @return {FileSnapShotCacheEntry}
     */
    set(key: string, entryContent: BasicFileSnapshot): FileSnapShotCacheEntry;
}
