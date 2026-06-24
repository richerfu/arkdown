/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/merge-node-assets.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * 将编译完的ets和js复制到同一个目录下
 */
export declare class MergeNodeAssets extends OhosHapTask {
    private readonly intermediatesOutPath;
    private readonly intermediatesAssetsPath;
    private _log;
    constructor(targetService: TargetTaskService);
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected beforeTask(): void;
    protected doTaskAction(): void;
    initTaskDepends(): void;
}
