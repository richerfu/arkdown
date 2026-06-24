/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/comparator/filesnapshot-detail-diff.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
export declare class FileSnapshotDetailDiff {
    addedNodes: BasicFileSnapshot[];
    updatedNodes: BasicFileSnapshot[];
    removedNodes: BasicFileSnapshot[];
}
