/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/debugger/build-hot-reload-resource.d.ts
 */
import { AbstractModuleHookTask } from '../hook/abstract-module-hook-task';
import { ModuleTaskService } from '../service/module-task-service';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 提供调用hot reload的前置task
 */
export declare class BuildHotReloadResource extends AbstractModuleHookTask {
    private _log;
    constructor(moduleTaskService: ModuleTaskService);
    registryAction: () => Function;
    initTaskDepends(taskTargetService: TargetTaskService): void;
}
