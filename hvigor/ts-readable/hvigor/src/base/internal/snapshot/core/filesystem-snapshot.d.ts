/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/core/filesystem-snapshot.d.ts
 */
import { FileSystemSnapshotHierarchyVisitor, SnapshotVisitResult } from '../generator/filesystem-snapshot-visitor';
import { FileType } from '../util/file-type';
import { Snapshot } from './snapshot';
/**
 * 快照抽象基类
 *
 * @since 2022/9/1
 */
export declare abstract class BasicFileSnapshot extends Snapshot {
    hashValue: string;
    name: string;
    path: string;
    type: FileType;
    isSymbolicLink: boolean;
    constructor(name?: string, path?: string, type?: FileType, isSymbolicLink?: boolean);
    /**
     * accept
     *
     * @param visitor
     */
    abstract accept(visitor: FileSystemSnapshotHierarchyVisitor): SnapshotVisitResult;
    /**
     * Generator 生成器 ，Depth First order
     */
    abstract walk(): IterableIterator<BasicFileSnapshot>;
    /**
     * equals
     *
     * @param other
     */
    abstract equals(other: BasicFileSnapshot): boolean;
    /**
     * isHashEquals
     *
     * @param other
     */
    isHashEquals(other: BasicFileSnapshot): boolean;
    /**
     * toJsonString
     */
    toJsonString(): string;
    [Symbol.iterator](): IterableIterator<BasicFileSnapshot>;
}
