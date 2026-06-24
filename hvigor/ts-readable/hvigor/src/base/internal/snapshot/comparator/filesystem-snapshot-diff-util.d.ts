/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/comparator/filesystem-snapshot-diff-util.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
import { FileSnapshotDetailDiff } from './filesnapshot-detail-diff';
import { FilesystemSnapshotDetailComparator } from './filesystem-snapshot-detail-comparator';
import { SnapshotDiffListener } from './snapshot-diff-listener';
/**
 * Snapshot DiffUtil
 *
 * @since 2022/9/1
 */
export declare class DiffUtil {
    static diff(oldSnapshot: BasicFileSnapshot, newSnapshot: BasicFileSnapshot, comparator: FilesystemSnapshotDetailComparator): boolean;
    /**
     * Snapshot diff
     *
     * @param oldSnapshot oldSnapshot
     * @param newSnapshot newSnapshot
     * @param comparator comparator
     */
    static diffWithDetail(oldSnapshot: BasicFileSnapshot, newSnapshot: BasicFileSnapshot, comparator: FilesystemSnapshotDetailComparator): FileSnapshotDetailDiff;
    /**
     * Diff with break 可以在任意结点提早结束比较 （参考 gradle）
     *
     * @param oldSnapshot oldSnapshot
     * @param newSnapshot newSnapshot
     * @param comparator comparator
     * @param diffListener diffListener
     */
    static diffWithBreak(oldSnapshot: BasicFileSnapshot, newSnapshot: BasicFileSnapshot, comparator: FilesystemSnapshotDetailComparator, diffListener: SnapshotDiffListener): boolean;
}
