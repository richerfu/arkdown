/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/module-incremental-skip-optimization.d.ts
 */
import { Project } from '@ohos/hvigor';
/**
 *  增量性能优化：在构建时对未改变模块的任务进行批量增量跳过，优化中间阶段构建耗时
 */
export declare class ModuleIncrementalSkipService {
    private readonly _log;
    /**
     * 进行模块级增量跳过的预处理
     * @param project
     */
    processModuleSkip(project: Project): void;
    /**
     * 本次构建时涉及所有模块，其产物是否存在
     * @param project 涉及工程
     */
    private getBuildModuleStat;
    /**
     * 计算并存储模块依赖关系
     * @param project 涉及工程
     */
    private getModuleDependMap;
    /**
     * 读取listener落盘的模块变更记录文件
     * @param changeRecordFilePath 模块变更记录文件路径
     */
    private readChangedFile;
}
