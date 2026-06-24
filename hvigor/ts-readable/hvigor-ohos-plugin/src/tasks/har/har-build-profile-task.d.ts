/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/har-build-profile-task.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHarTask } from "../task/ohos-har-task";
export interface HarBuildProfileData {
    HAR_VERSION: string;
    BUILD_MODE_NAME: string;
    DEBUG: boolean;
    TARGET_NAME: string;
}
export declare class HarBuildProfileTask extends OhosHarTask {
    private _log;
    protected readonly harTaskService: TargetTaskService;
    protected harBuildProfileData: HarBuildProfileData | undefined;
    constructor(taskService: TargetTaskService);
    protected initDefaultData(): void;
    private get appJsonPath();
    private get projectProfilePath();
    private get harBuildProfilePath();
    private get buildModeName();
    private get buildMode();
    private get buildProfileFields();
    private get harVersion();
    private get harTargetName();
    initTaskDepends(): void;
    protected doTaskAction(): void;
}
