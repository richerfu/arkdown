/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/assemble/assemble-hqf.d.ts
 */
import { ModuleTaskService } from '../../service/module-task-service';
import { TargetTaskService } from '../../service/target-task-service';
import { AbstractModuleHookTask } from '../abstract-module-hook-task';
export declare class AssembleHqf extends AbstractModuleHookTask {
    static isAdd: boolean;
    static versionCode: number | undefined;
    constructor(moduleService: ModuleTaskService);
    initTaskDepends(taskTargetService: TargetTaskService): void;
    registryAction: () => () => Promise<void>;
}
