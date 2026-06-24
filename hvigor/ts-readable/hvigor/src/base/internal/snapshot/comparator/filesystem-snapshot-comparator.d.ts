/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/comparator/filesystem-snapshot-comparator.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
/**
 * Filesystem snapshot comparator
 *
 * @since 2022/9/1
 */
export interface FilesystemSnapshotComparator {
    compare(oldSnapshot: BasicFileSnapshot, newSnapshot: BasicFileSnapshot): boolean;
}
export declare class DefaultFileSystemSnapshotComparator implements FilesystemSnapshotComparator {
    compare(oldSnapshot: BasicFileSnapshot, newSnapshot: BasicFileSnapshot): boolean;
}
