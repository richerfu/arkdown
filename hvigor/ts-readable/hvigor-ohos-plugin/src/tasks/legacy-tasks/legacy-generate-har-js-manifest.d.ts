/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-har-js-manifest.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * 为har生成一个假的manifest
 *
 * @since 2022/3/1
 */
export declare class LegacyGenerateHarJsManifest extends OhosHapTask {
    private _log;
    private previewPage;
    private readonly _moduleModel;
    private readonly _configOpt;
    private readonly _minPlatformVersion;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    private static getWindowObj;
    declareInputs(): Map<string, TaskInputValue>;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): void;
    private getManifestPages;
    private generateJsonInfo;
}
