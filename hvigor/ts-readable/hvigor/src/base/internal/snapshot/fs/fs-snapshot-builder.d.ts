/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/fs/fs-snapshot-builder.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
import { FsEvent } from './fs-event';
export interface FsSnapshotBuilder {
    /**
     * 处理遇见文件夹的情景
     *
     * @param info
     */
    enterDirectory(ev: FsEvent): void;
    /**
     * 处理遇见文件的场景
     * @param info
     */
    visitEntry(ev: FsEvent): void;
    /**
     * 文件夹访问完成时调用
     */
    leaveDirectory(fileev: FsEvent): void;
    /**
     * 返回Snapshot实例
     */
    getResult(): BasicFileSnapshot | undefined;
}
