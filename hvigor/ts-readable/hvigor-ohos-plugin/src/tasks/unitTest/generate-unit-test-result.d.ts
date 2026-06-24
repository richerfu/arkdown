/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/generate-unit-test-result.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosModuleTask } from '../task/ohos-module-task';
/**
 * 调用ohos/coverage接口，生成测试覆盖率报告
 *
 * @2023/12/29
 */
export declare class GenerateUnitTestResult extends OhosModuleTask {
    private logger;
    constructor(taskService: TargetTaskService);
    /**
     * 返回HarmonyOS sdk下面openharmony目录下的 previewer 路径
     */
    private getHmosPreviewerPath;
    /**
     * 返回HarmonyOS sdk下面hms目录下的 previewer 路径
     */
    private getHmsPreviewerPath;
    /**
     * 返回OpenHarmony sdk下面的 previewer 路径
     */
    private getOhosPreviewerPath;
    private getPreviewerPath;
    doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
