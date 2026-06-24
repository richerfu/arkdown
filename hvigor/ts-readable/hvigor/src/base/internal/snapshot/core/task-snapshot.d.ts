/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/core/task-snapshot.d.ts
 */
import { ProjectCacheService } from '../../cache/project-cache-service';
import { FsOptions } from '../util/file-set';
import { ValueEntry } from '../util/task-input-value-entry';
/**
 * 任务快照数据结构
 *
 * @since 2022/8/31
 */
export interface TaskSnapshot {
    /**
     * 根据任务信息组成标识key, 用作定位
     */
    getKey(): string;
    getTaskName(): string;
    getProjectName(): string;
    getModuleName(): string;
    isSuccessful(): boolean;
    getInputs(): ValueEntry[];
    getInputFiles(): Map<string, FsOptions>;
    getOutputFiles(): Map<string, FsOptions>;
    compareTo(other: TaskSnapshot): boolean;
    stringify(): string;
    setExecutedSuccessful(): void;
    updateFilesOptions(projectCacheService: ProjectCacheService): void;
    updateCacheFilesOptions(projectCacheService: ProjectCacheService): void;
}
