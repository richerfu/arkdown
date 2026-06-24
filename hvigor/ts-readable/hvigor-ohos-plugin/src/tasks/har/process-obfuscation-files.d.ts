/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/process-obfuscation-files.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHarTask } from '../task/ohos-har-task';
/**
 * 调用etsLoader生成合并的obfuscation.txt
 */
export declare class ProcessObfuscationFiles extends OhosHarTask {
    private _log;
    constructor(taskService: TargetTaskService);
    taskShouldDo(): boolean;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
