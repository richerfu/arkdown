/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/preview-hook-compile-resource.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * 预览时资源编译处理是否支持Restool增量方式编译的hook任务
 *
 * @since 2022/10/12
 */
export declare class PreviewHookCompileResource extends OhosHapTask {
    constructor(taskService: TargetTaskService);
    protected doTaskAction(): void;
    initTaskDepends(): void;
}
