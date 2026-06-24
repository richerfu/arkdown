/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/replace-sourcemap-in-coverage-mode.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosModuleTask } from '../task/ohos-module-task';
/**
 * 覆盖率场景下替换sourceMap
 *
 * @since 2024-07-25
 */
export declare class ReplaceSourcemapInCoverageMode extends OhosModuleTask {
    unitTestSourceMapPath: string;
    defaultSourceMapPath: string;
    constructor(taskService: TargetTaskService);
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
