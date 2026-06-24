/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/hook/legacy-hook-compile-resource.d.ts
 */
import { TargetTaskService } from '../../service/target-task-service';
import { OhosHapTask } from '../../task/ohos-hap-task';
import { TaskDetails } from '@ohos/hvigor';
/**
 * 增加一个忽略feature和entry差异的编译资源的hook任务，用于挂接feature的多个entryModules的任务
 *
 * @since 2022/9/8
 */
export declare class LegacyHookCompileResource extends OhosHapTask {
    constructor(taskService: TargetTaskService);
    protected doTaskAction(): void;
    initTaskDepends(): void;
}
export declare function getTaskRealName(taskDetail: TaskDetails, entryModuleName?: string): TaskDetails;
