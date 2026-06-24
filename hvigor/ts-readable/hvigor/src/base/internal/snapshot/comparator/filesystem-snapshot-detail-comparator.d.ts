/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/comparator/filesystem-snapshot-detail-comparator.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
export interface FilesystemSnapshotDetailComparator {
    compare(previous: BasicFileSnapshot, current: BasicFileSnapshot): number;
}
export declare class FileSystemSnapshotNameComparator implements FilesystemSnapshotDetailComparator {
    compare(previous: BasicFileSnapshot, current: BasicFileSnapshot): number;
}
export declare class FileSystemSnapshotHashValueComparator implements FilesystemSnapshotDetailComparator {
    compare(previous: BasicFileSnapshot, current: BasicFileSnapshot): number;
}
