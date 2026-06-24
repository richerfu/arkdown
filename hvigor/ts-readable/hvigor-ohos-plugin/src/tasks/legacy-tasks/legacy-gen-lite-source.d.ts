/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-gen-lite-source.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * 生成lite的资源文件
 */
export declare class LegacyGenLiteSource extends OhosHapTask {
    private _logger;
    protected syscapJsonPath: string | undefined;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    protected doTaskAction(): Promise<void>;
    private genLiteSource;
    protected beforeTask(): Promise<void>;
}
