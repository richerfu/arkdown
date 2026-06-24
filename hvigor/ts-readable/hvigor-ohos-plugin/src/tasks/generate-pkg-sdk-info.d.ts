/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-pkg-sdk-info.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
export declare class GeneratePkgSdkInfo extends OhosHapTask {
    private pkgSdkInfoMap;
    private pkgSdkInfoPath;
    constructor(taskService: TargetTaskService);
    private get useNormalizedOHMUrl();
    beforeAlwaysAction(): Promise<void>;
    protected beforeTask(): void;
    declareInputs(): Map<string, TaskInputValue>;
    declareOutputFiles(): FileSet;
    taskShouldDo(): boolean;
    protected doTaskAction(): void;
    initTaskDepends(): void;
    private initPkgSdkInfo;
    private getVersion;
}
