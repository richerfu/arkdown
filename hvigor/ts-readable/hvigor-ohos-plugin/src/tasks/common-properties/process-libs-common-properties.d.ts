/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/common-properties/process-libs-common-properties.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
export declare class ProcessLibsCommonProperties extends OhosHapTask {
    protected constructor(taskService: TargetTaskService, taskDetails: TaskDetails);
    protected get libsPattern(): "**/*.so*(.*[0-9])" | "**/*";
    protected getLibsInfo(libsPath: string): string[];
    protected doTaskAction(): void;
    initTaskDepends(): void;
}
