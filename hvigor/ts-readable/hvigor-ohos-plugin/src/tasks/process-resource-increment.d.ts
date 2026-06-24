/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-resource-increment.d.ts
 */
import { AbstractProcessResourceIncrement } from './abstract/abstract-process-resource-increment';
import { TargetTaskService } from './service/target-task-service';
/**
 * Stage 模型用于处理和生成Restool增量方式的编译、链接命令参数
 *
 * @since 2022/09/22
 */
export declare class ProcessResourceIncrement extends AbstractProcessResourceIncrement {
    protected FIRST_SUPPORT_RESTOOL_IDSMAP_APIVERSION: number;
    protected idsMapDir: string;
    constructor(taskService: TargetTaskService);
    initCommandBuilder(): void;
}
