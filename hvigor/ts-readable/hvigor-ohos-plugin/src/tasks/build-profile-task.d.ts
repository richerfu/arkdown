/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/build-profile-task.d.ts
 */
import { TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
export interface BuildProfileData {
    BUNDLE_NAME: string;
    BUNDLE_TYPE: string;
    TARGET_NAME: string;
    PRODUCT_NAME: string;
    BUILD_MODE_NAME: string;
    DEBUG: boolean;
    VERSION_CODE: number;
    VERSION_NAME: string;
    BUILD_VERSION?: string;
}
export declare class BuildProfileTask extends OhosHapTask {
    private _log;
    protected readonly buildTaskService: TargetTaskService;
    protected buildProfileData: BuildProfileData | undefined;
    constructor(taskService: TargetTaskService);
    protected initDefaultData(): void;
    private get buildMode();
    private get buildModeName();
    private get buildProfileFields();
    private get projectOhosConfigAppOpt();
    declareInputs(): Map<string, TaskInputValue>;
    private get appResJsonPath();
    private get projectProfilePath();
    private get buildProfilePath();
    private get bundleName();
    private get bundleType();
    private get versionCode();
    private get versionName();
    private get productName();
    private get debug();
    initTaskDepends(): void;
    protected doTaskAction(): void;
}
