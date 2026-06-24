/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/build-unit-test-hook.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * unit test编译资源
 *
 * @2023/4/24
 */
export declare class BuildUnitTestHook extends OhosHapTask {
    protected syscapJsonPath: string;
    protected isFaMode: boolean;
    constructor(targetService: TargetTaskService, isFaMode: boolean);
    protected doTaskAction(): void;
    initTaskDepends(): void;
    private getLegacyUnitTestCompileRes;
    private createTaskAndDepends;
}
