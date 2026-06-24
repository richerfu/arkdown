/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/core/directory-snapshot.d.ts
 */
import { FileSystemSnapshotHierarchyVisitor, SnapshotVisitResult } from '../generator/filesystem-snapshot-visitor';
import { BasicFileSnapshot } from './filesystem-snapshot';
/**
 * 文件夹快照
 *
 * @since 2022/9/1
 */
export declare class DirectorySnapshot extends BasicFileSnapshot {
    children: BasicFileSnapshot[];
    accept(visitor: FileSystemSnapshotHierarchyVisitor): SnapshotVisitResult;
    walk(): IterableIterator<BasicFileSnapshot>;
    equals(other: BasicFileSnapshot): boolean;
}
