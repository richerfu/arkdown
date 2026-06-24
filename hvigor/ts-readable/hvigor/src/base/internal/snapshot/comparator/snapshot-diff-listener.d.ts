/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/comparator/snapshot-diff-listener.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
/**
 * Snapshot diff listener
 *
 * @since 2022/9/1
 */
export interface SnapshotDiffListener {
    /**
     * Removed node listener
     *
     * @param previous
     */
    removed(previous: BasicFileSnapshot): boolean;
    /**
     * Added node listener
     *
     * @param current
     */
    added(current: BasicFileSnapshot): boolean;
    /**
     * Updated node listener
     *
     * @param previous
     * @param current
     */
    updated(previous: BasicFileSnapshot, current: BasicFileSnapshot): boolean;
}
