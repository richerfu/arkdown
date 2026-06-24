/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-js-manifest.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * 遍历project的module信息并生成JsManifest.json文件
 *
 * @since 2022/1/19
 */
export declare class LegacyGenerateJsManifest extends OhosHapTask {
    private _log;
    private readonly previewSrcPath;
    private readonly unitTestSrcPath;
    private readonly isPreview;
    private readonly previewPage;
    private readonly unitTestPage;
    private readonly isUnitTest;
    private readonly _moduleModel;
    private readonly _abilityModels;
    private readonly _configOpt;
    private readonly _minPlatformVersion;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    declareInputs(): Map<string, TaskInputValue>;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): void;
    private getUnitTestManifestPage;
    private getManifestPages;
    private getAbilityPages;
    private generateJsonInfo;
    private static findAppName;
    private getWindowObj;
}
