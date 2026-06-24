/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-loader-json.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { TaskInputValue } from '@ohos/hvigor';
import { AbstractGenerateLoaderJson } from '../abstract-generate-loader-json';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 生成fa模型编译需要的loader.json
 */
export declare class LegacyGenerateLoaderJson extends AbstractGenerateLoaderJson {
    private readonly outputAceLoaderJsonArray;
    private readonly multiResourceBuildArray;
    private readonly isWorkerArray;
    declareInputs(): Map<string, TaskInputValue>;
    declareOutputFiles(): FileSet;
    initTaskDepends(): void;
    constructor(taskService: TargetTaskService);
    private shouldLoaderWorker;
    protected doTaskAction(): void;
    private getMultiResourceBuild;
    /**
     * loader.json中的pages与manifest.json中的pages保持一致
     * loader.json: build/{default}/intermediates/loader/{default}/{MainAbility}/loader.json
     * manifest.json: build/{default}/intermediates/manifest/{default}/{MainAbility}/manifest.json
     *
     * @param targetData targetData
     * @param abilityInfo current page ability info
     * @private
     */
    private getManifestPage;
}
