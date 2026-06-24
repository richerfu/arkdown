/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/service/snapshot-generator-service.d.ts
 */
import { IncrementalTask } from '../../../external/task/incremental-task';
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
import { TaskSnapshot } from '../core/task-snapshot';
import { FileSystemSnapshotGenerator } from '../generator/filesystem-snapshot-generator';
import { TaskSnapshotGenerator } from '../generator/task-snapshot-generator';
import { FsOptions } from '../util/file-set';
import { Snapshot } from '../core/snapshot';
/**
 * Snapshot generator 外部统一入口单例类
 *
 * @since 2022/9/1
 */
export declare class SnapshotGeneratorService {
    private static _instance;
    private defaultFileSystemSnapshotGenerator;
    private taskGenerator;
    private buildInfoGenerator;
    private constructor();
    static getInstance(): SnapshotGeneratorService;
    generateTaskSnapshot(task: IncrementalTask, generator?: TaskSnapshotGenerator): TaskSnapshot;
    /**
     * generateFileSystemSnapshot
     *
     * @param filepath file or directory path
     * @param options FsOptions for filter file
     * @param fileSystemSnapshotGenerator generator
     */
    generateFileSystemSnapshot(filepath: string, options?: FsOptions, fileSystemSnapshotGenerator?: FileSystemSnapshotGenerator): BasicFileSnapshot;
    loadTaskSnapshotFromJson(jsonStr: string): TaskSnapshot;
    loadBuildInfoSnapshotCacheFromJson(text: string): Map<string, Snapshot>;
    serializeBuildInfoSnapshotCacheToJson(): string;
    loadTaskSnapshotCacheFromJson(text: string): Map<string, TaskSnapshot>;
    serializeTaskSnapshotCacheToJson(collection: Map<string, TaskSnapshot>): string;
    /**
     * json string 转 FileSystem snapshot
     *
     * @param jsonStr
     */
    loadFileSystemSnapshotFromJson(jsonStr: string): BasicFileSnapshot | undefined;
    /**
     * json string 转 Map<string, BasicFileSnapshot>
     *
     * @param jsonStr
     */
    loadFileSystemSnapshotCacheFromJson(jsonStr: string): Map<string, BasicFileSnapshot> | undefined;
    /**
     * serialize map of  BasicFileSnapshot to json string
     *
     * @param map
     */
    serializeFileSystemSnapshotToJson(map: Map<string, BasicFileSnapshot>): string;
}
