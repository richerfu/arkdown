/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/debugger/hot-reload-build.d.ts
 */
import { AbstractModuleHookTask } from '../hook/abstract-module-hook-task';
import { ModuleTaskService } from '../service/module-task-service';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 调试方构建
 *
 * @since 2022/11/29
 */
export declare class HotReloadBuild extends AbstractModuleHookTask {
    constructor(moduleTaskService: ModuleTaskService);
    initTaskDepends(taskTargetService: TargetTaskService): void;
}
