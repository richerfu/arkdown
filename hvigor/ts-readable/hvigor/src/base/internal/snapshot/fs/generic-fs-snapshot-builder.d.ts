/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/fs/generic-fs-snapshot-builder.d.ts
 */
import { FileSnapshot } from '../core/file-snapshot';
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
import { FsEvent } from './fs-event';
import { FsSnapshotBuilder } from './fs-snapshot-builder';
/**
 * 快照生成器
 * 仅hash文件信息, 未hash文件内容
 *
 * @since 2022/08/27
 */
export declare class GenericFsSnapshotBuilder implements FsSnapshotBuilder {
    private directoryStack;
    private directorySnapshot;
    enterDirectory(ev: FsEvent): void;
    visitEntry(ev: FsEvent): void;
    leaveDirectory(ev: FsEvent): void;
    handleEntry(snapshot: BasicFileSnapshot): void;
    private completeDirectory;
    hashItem(snapshot: FileSnapshot): string;
    getResult(): BasicFileSnapshot | undefined;
    private assembleSnapshot;
    private peekLast;
}
