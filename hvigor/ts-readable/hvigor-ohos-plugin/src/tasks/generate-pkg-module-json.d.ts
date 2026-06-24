/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-pkg-module-json.d.ts
 */
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
export declare class GeneratePkgModuleJson extends OhosHapTask {
    protected readonly jsonPath: string;
    protected readonly packageHapJsonPath: string;
    private readonly preloadSoJsonPath;
    private shouldDeduplicateHar;
    constructor(taskService: TargetTaskService);
    private ensureAppStartup;
    /**
     * har包去重时hsp的module.json中添加deduplicateHar字段来标识去重
     */
    private writeDeduplicateHarField;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
