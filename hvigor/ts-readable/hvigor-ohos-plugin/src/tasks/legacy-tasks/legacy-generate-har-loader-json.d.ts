/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-har-loader-json.d.ts
 */
import { AbstractGenerateLoaderJson } from '../abstract-generate-loader-json';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 生成fa模型编译需要的loader.json
 */
export declare class LegacyGenerateHarLoaderJson extends AbstractGenerateLoaderJson {
    private readonly aceLoaderJson;
    declareOutputFiles(): import("@ohos/hvigor").FileSet;
    initTaskDepends(): void;
    constructor(taskService: TargetTaskService);
    protected doTaskAction(): void;
}
