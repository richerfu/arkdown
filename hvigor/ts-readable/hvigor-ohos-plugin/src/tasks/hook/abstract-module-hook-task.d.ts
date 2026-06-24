/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/abstract-module-hook-task.d.ts
 */
import { DefaultTask, TaskDetails } from '@ohos/hvigor';
import { ModuleTaskService } from '../service/module-task-service';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 继承此类的都是以module为基础，提供的hook task，不区分不同的target
 *
 * @since 2022/8/11
 */
export declare abstract class AbstractModuleHookTask extends DefaultTask {
    protected readonly _moduleService: ModuleTaskService;
    protected readonly _isFaMode: boolean;
    protected constructor(moduleService: ModuleTaskService, isFaMode: boolean, taskName: TaskDetails);
    registryAction: () => Function;
    abstract initTaskDepends(taskTargetService: TargetTaskService): void;
}
