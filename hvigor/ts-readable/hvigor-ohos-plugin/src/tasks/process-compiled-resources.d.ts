/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-compiled-resources.d.ts
 */
import { OhosHapTask } from './task/ohos-hap-task';
import { TargetTaskService } from './service/target-task-service';
/**
 * 打包hap之前，以下场景，需要对资源进行二次处理，基于hvigor的增量机制，先拷贝resource出来，再修改：
 * 1.开启preloadSystemSo开关
 *
 * @since 2025/7/7
 */
export declare class ProcessCompiledResources extends OhosHapTask {
    private readonly resourcePath;
    private readonly preloadSoJsonPath;
    constructor(taskService: TargetTaskService);
    taskShouldDo(): boolean;
    protected beforeTask(): void;
    protected processPreloadSystemSoJson(): void;
    initTaskDepends(): void;
    protected doTaskAction(): void;
}
