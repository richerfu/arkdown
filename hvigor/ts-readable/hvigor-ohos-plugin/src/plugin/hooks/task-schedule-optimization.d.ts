/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/task-schedule-optimization.d.ts
 */
import { Project } from '@ohos/hvigor';
/**
 *  计算CompileArkTS依赖链上的任务
 */
export declare class TaskScheduleOptimizationService {
    private static readonly SUPPORTED_ASSEMBLE_TASKS;
    private readonly _log;
    private project;
    private visitedTasks;
    constructor(project: Project);
    /**
     * 检查入口任务是否支持调度优化
     */
    private checkSupportedEntryTasks;
    /**
     * 工程下每个节点/模块进行编译任务链计算汇总
     * 遍历工程的子模块，提取每个子模块的编译任务，并处理这些任务的依赖关系
     * 最终将所有依赖任务设置到编译依赖数组中
     */
    computeAndSetCompileDependencies(): void;
    /**
     * 过滤得到编译任务
     *
     * @param subModuleNode 子模块节点
     */
    private extractCompileTasks;
    /**
     * 广度优先汇总编译相关依赖任务
     *
     * @param queue 任务队列
     */
    private processTaskQueue;
    /**
     * 依赖任务入队
     *
     * @param queue 任务队列
     * @param task 当前任务
     */
    private addTaskDependenciesToQueue;
    /**
     * 从DAG中获取任务的依赖任务路径
     *
     * @param task 当前任务
     * @return 返回任务的依赖列表
     */
    private getSafeTaskDependencies;
}
