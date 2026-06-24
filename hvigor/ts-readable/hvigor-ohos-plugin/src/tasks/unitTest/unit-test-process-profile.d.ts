/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/unit-test-process-profile.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * UnitTestBuild场景处理构建中间产物module.json文件
 * 1.增加TestAbility入口
 *
 * @since 2023/9/11
 */
export declare class UnitTestProcessProfile extends OhosHapTask {
    private readonly processProfileModuleJsonPath;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    protected doTaskAction(): void;
}
