/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-process-resource.d.ts
 */
import { AbstractProcessResource } from '../abstract/abstract-process-resource';
import { TargetTaskService } from '../service/target-task-service';
/**
 * Fa 模型用于处理和生成用文件方式编译资源的中间文件
 *
 * @since 2022/9/8
 */
export declare class LegacyProcessResource extends AbstractProcessResource {
    constructor(taskService: TargetTaskService, entryModuleName?: string);
    protected initTaskDependsForOtherModule(): void;
    initTaskDepends(): void;
    /**
     * Feature添加Entry依赖
     * FA模型 Feature模块依赖Entry模块资源编译完成
     *
     * @private
     */
    private featureAddEntryDpends;
    initCommandBuilder(): void;
    protected doTaskAction(): void;
    /**
     * 获取Entry模块资源编译结果目录
     *
     * @param {ModuleTargetData} targetData 模块target对象
     * @param {string} entryName entry模块名称
     * @private
     */
    private getEntryCompileResouceDir;
}
