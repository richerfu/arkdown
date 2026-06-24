/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/generator/default-filesystem-snapshot-generator.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
import { FileMetaHasher } from '../util/file-hasher';
import { FsOptions } from '../util/file-set';
import { FileSystemSnapshotGenerator } from './filesystem-snapshot-generator';
export declare class DefaultFilesystemSnapshotGenerator implements FileSystemSnapshotGenerator {
    private readonly fileHasher;
    constructor(fileHasher?: FileMetaHasher);
    private parseFileSystemSnapshot;
    /**
     * Snapshot a file or directory
     *
     * @param filepath filepath
     * @param options FsOptions for filter file
     */
    snapshot(filepath: string, options?: FsOptions): BasicFileSnapshot;
    loadSnapshotFromJson(jsonStr: string): BasicFileSnapshot | undefined;
    loadSnapshotCacheFromJson(jsonStr: string): Map<string, BasicFileSnapshot> | undefined;
    serializeSnapshotToJson(map: Map<string, BasicFileSnapshot>): string;
}
