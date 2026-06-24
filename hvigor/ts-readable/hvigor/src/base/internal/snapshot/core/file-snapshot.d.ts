/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/core/file-snapshot.d.ts
 */
import { FileSystemSnapshotHierarchyVisitor, SnapshotVisitResult } from '../generator/filesystem-snapshot-visitor';
import { FileMetaData } from '../util/file-metadata';
import { FileType } from '../util/file-type';
import { BasicFileSnapshot } from './filesystem-snapshot';
/**
 * 单个文件快照
 *
 * @since 2022/9/1
 */
export declare class FileSnapshot extends BasicFileSnapshot {
    fileMetaData: FileMetaData;
    constructor(name: string, path: string, type: FileType, isSymbolicLink: boolean, fileMetaData: FileMetaData);
    accept(visitor: FileSystemSnapshotHierarchyVisitor): SnapshotVisitResult;
    walk(): IterableIterator<BasicFileSnapshot>;
    equals(other: BasicFileSnapshot): boolean;
}
