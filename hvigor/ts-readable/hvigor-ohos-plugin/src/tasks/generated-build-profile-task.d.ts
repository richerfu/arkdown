/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generated-build-profile-task.d.ts
 */
import { AbstractModuleHookTask } from './hook/abstract-module-hook-task';
import { ModuleTaskService } from './service/module-task-service';
import { TargetTaskService } from './service/target-task-service';
export declare class GeneratedBuildProfileTask extends AbstractModuleHookTask {
    private readonly isHar;
    constructor(moduleService: ModuleTaskService, isHar: boolean);
    initTaskDepends(taskTargetService: TargetTaskService): void;
}
