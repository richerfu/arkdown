/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/fs/default-fs-snapshot-generator.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
import { FileSystemSnapshotGenerator } from '../generator/filesystem-snapshot-generator';
import { FsOptions } from '../util/file-set';
export declare class DefaultFsSnapshotGenerator implements FileSystemSnapshotGenerator {
    /**
     * Snapshot a file or directory
     *
     * @param location location
     * @param options
     */
    snapshot(location: string, options?: FsOptions): BasicFileSnapshot;
    loadSnapshotCacheFromJson(jsonStr: string): Map<string, BasicFileSnapshot> | undefined;
    loadSnapshotFromJson(jsonStr: string): BasicFileSnapshot | undefined;
    serializeSnapshotToJson(map: Map<string, BasicFileSnapshot>): string;
}
