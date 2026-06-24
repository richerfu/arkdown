/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/har/legacy-process-oh-package-json.d.ts
 */
import { TargetTaskService } from '../../service/target-task-service';
import { OhosHarTask } from '../../task/ohos-har-task';
/**
 * har模块需要处理指定resource目录
 * 当前该任务执行处理很快,先不需要做增量
 */
export declare class LegacyProcessOHPackageJson extends OhosHarTask {
    private readonly generateOhPackageJsonFilePath;
    constructor(taskService: TargetTaskService);
    protected doTaskAction(): void;
    initTaskDepends(): void;
}
