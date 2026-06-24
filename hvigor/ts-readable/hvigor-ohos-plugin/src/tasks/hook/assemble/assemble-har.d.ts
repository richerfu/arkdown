/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/assemble/assemble-har.d.ts
 */
import { ModuleTaskService } from '../../service/module-task-service';
import { TargetTaskService } from '../../service/target-task-service';
import { AbstractModuleHookTask } from '../abstract-module-hook-task';
/**
 * 组装Hap打包的任务流
 *
 * @since 2022/1/20
 */
export declare class AssembleHar extends AbstractModuleHookTask {
    constructor(moduleService: ModuleTaskService, isFaMode: boolean);
    initTaskDepends(taskTargetService: TargetTaskService): void;
}
