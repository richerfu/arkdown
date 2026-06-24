/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/task-initializer.d.ts
 */
import { AbstractModulePlugin } from '../plugin/common/abstract-module-plugin';
import { TargetTaskService } from '../tasks/service/target-task-service';
import { TaskCreatorManager } from '../tasks/task-creator';
export declare class TaskInitializer {
    /**
     *
     * @param plugin
     */
    static initializeHap(plugin: AbstractModulePlugin): TaskCreatorManager;
    static initializeHsp(plugin: AbstractModulePlugin): TaskCreatorManager;
    /**
     *
     * @param plugin
     */
    static initializeHar(plugin: AbstractModulePlugin): TaskCreatorManager;
    private static commonHap;
    private static initializeCommonHookTasks;
    private static initializeTargetTasks;
    /**
     * target任务初始化入口
     *
     * @param service
     * @param creatorManager
     * @private
     */
    static targetInitialize(service: TargetTaskService, creatorManager: TaskCreatorManager): void;
    private static hapTasksInitialize;
    private static hspTasksInitialize;
    private static harTasksInitialize;
    /**
     * hap/hsp所需任务, har不需要
     *
     * @param service
     * @param creatorManager
     * @private
     */
    private static tasksInitialize;
    /**
     * hap/hsp/har所需任务
     *
     * @param service
     * @param creatorManager
     * @private
     */
    private static initializeCommonTargetTasks;
}
