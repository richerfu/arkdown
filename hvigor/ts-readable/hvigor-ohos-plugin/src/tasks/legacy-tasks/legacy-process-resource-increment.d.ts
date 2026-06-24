/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-process-resource-increment.d.ts
 */
import { AbstractProcessResourceIncrement } from '../abstract/abstract-process-resource-increment';
import { TargetTaskService } from '../service/target-task-service';
/**
 * FA 模型用于处理和生成Restool增量方式的编译、链接命令参数
 *
 * @since 2022/09/22
 */
export declare class LegacyProcessResourceIncrement extends AbstractProcessResourceIncrement {
    constructor(taskService: TargetTaskService, entryModuleName?: string);
    initCommandBuilder(): void;
}
